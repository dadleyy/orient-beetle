#pragma once

#include <DNSServer.h>
#include <Preferences.h>
#include <WiFi.h>
#include <WiFiAP.h>
#include <WiFiClient.h>
#include <WiFiServer.h>

#include <optional>
#include <variant>

#include "esp32-hal-log.h"

namespace wifievents {

constexpr static const char *CONNECTION_PREFIX = "GET /connect?";
constexpr static uint8_t MAX_CLIENT_BLANK_READS = 5;
constexpr static uint16_t SERVER_BUFFER_CAPACITY = 1024;
constexpr static uint16_t MAX_HEADER_SIZE = 512;
constexpr static uint16_t MAX_NETWORK_CREDENTIAL_SIZE = 256;
constexpr static uint16_t MAX_PENDING_CONNECTION_ATTEMPTS = 20;

class Events final {
 public:
  // The things that can happen during an update frame
  enum EMessage {
    AttemptingConnection = 0,
    WaitingForCredentials,
    Connected,
    FailedConnection,
    Disconnected,
    ConnectionInterruption,
    ConnectionResumed,
  };

  explicit Events(std::tuple<const char *, const char *> ap)
      : _mode(std::in_place_type_t<Configuring>()),
        _ap_config(ap),
        _ssid(
            std::make_shared<std::array<char, MAX_NETWORK_CREDENTIAL_SIZE>>()),
        _password(
            std::make_shared<std::array<char, MAX_NETWORK_CREDENTIAL_SIZE>>()),
        _preferences(std::make_shared<Preferences>()),
        _visitor(Visitor{&_ap_config, 0, _ssid, _password, _preferences}) {
    _ssid->fill('\0');
    _password->fill('\0');
  }

  ~Events() = default;

  void begin(void) {
    log_i("wifi events preparing non-volatile storage");
    _preferences->begin("beetle-wifi", false);
  }

  std::optional<EMessage> update(uint32_t current_time) {
    _visitor._time = current_time;

    log_d("checking wifi state");
    auto [next, update] = std::visit(_visitor, std::move(_mode));
    _mode = std::move(next);
    log_d("wifi update complete");

    return update;
  }

  uint8_t attempt(void) { return 0; }

 private:
  class Visitor;

  class Connecting final {
    friend class Visitor;
    uint32_t attempt = 0;
    uint32_t attempt_time = 0;
  };

  struct Active final {
    bool ok = false;
    uint16_t interrupts = 0;
  };

  // Whenever we are without an active connection attempt or established
  // connection, the underlying state will deal with creating an access point
  // that responds with a "capture portal" where the user will enter in their
  // real access point credentials.
  class Configuring final {
    friend class Visitor;

   public:
    Configuring()
        : _server(std::make_unique<WiFiServer>(80)),
          _dns(std::make_unique<DNSServer>()) {}

    ~Configuring() {
      if (_server) {
        log_e("wifi manager terminating server state");
        _server->stop();
        log_i("wifi manager successfully terminated server state");
      } else {
        log_d("wifi manager has no wifi server to tear down");
      }

      if (_dns) {
        log_e("wifi manager terminating dns state");
        _dns->stop();
        log_i("wifi manager successfully terminated DNS server state");
      } else {
        log_d("wifi manager has no DNS server to tear down");
      }
    }

    // This is not a copyable resource; copying the wifi server is not defined
    // behavior.
    Configuring(Configuring &) = delete;
    Configuring &operator=(Configuring &) = delete;
    Configuring(const Configuring &) = delete;
    Configuring &operator=(const Configuring &) = delete;

    Configuring(Configuring &&c) = default;
    Configuring &operator=(Configuring &&c) = default;

   private:
    // While receiving data on our wifi server, these states represent what we
    // are in the process of doing.
    enum ERequestParsingMode {
      None = 0,
      StartNetwork = 1,
      NetworkValue = 2,
      PasswordStart = 3,
      PasswordValue = 3,
      Done = 4,
      Failed = 5,
    };

    std::unique_ptr<WiFiServer> _server;
    std::unique_ptr<DNSServer> _dns;
    bool _initialized = false;
  };

  using State = std::variant<Configuring, Connecting, Active>;

  struct Visitor final {
    std::tuple<State, std::optional<EMessage>> operator()(
        Configuring &&configuring) {
      uint8_t fields_set = 0;

      std::optional<EMessage> initial =
          configuring._initialized
              ? std::nullopt
              : std::make_optional(EMessage::WaitingForCredentials);

      if (_preferences->isKey("ssid") && _preferences->isKey("password")) {
        _preferences->getString("ssid", _ssid->data(),
                                MAX_NETWORK_CREDENTIAL_SIZE);
        _preferences->getString("password", _password->data(),
                                MAX_NETWORK_CREDENTIAL_SIZE);

        log_i("wifi attempting stored credentials (ssid: %d, password: %d)",
              strlen((char *)_ssid.get()), strlen((char *)_password.get()));

        return std::make_tuple(Connecting{}, EMessage::AttemptingConnection);
      }

      if (!configuring._initialized) {
        const char *capture_ssid = std::get<0>(*_ap_config);
        const char *capture_pass = std::get<1>(*_ap_config);

        log_i("intializing access point with ssid='%s' password='%s'",
              capture_ssid, capture_pass);

        WiFi.softAP(capture_ssid, capture_pass, 7, 0, 1);
        IPAddress address = WiFi.softAPIP();

        log_i("access point (router) ip address: %s", address.toString());
        configuring._server->begin();
        configuring._dns->start(53, "*", address);
        configuring._initialized = true;
      }

      configuring._dns->processNextRequest();
      WiFiClient client = configuring._server->available();

      if (!client) {
        if (_time - _last_debug > 3000) {
          log_i("no client connected for configuration yet (%d vs %d)", _time,
                _last_debug);
          _last_debug = _time;
        }

        return std::make_tuple(State(std::move(configuring)), initial);
      }

      _ssid->fill('\0');
      _password->fill('\0');

      extern const char index_html[] asm("_binary_embeds_index_http_start");
      extern const char index_end[] asm("_binary_embeds_index_http_end");
      log_d("loaded index (%d bytes)", index_end - index_html);

      uint16_t cursor = 0, field = 0;
      uint8_t noreads = 0;

      Configuring::ERequestParsingMode method =
          Configuring::ERequestParsingMode::None;

      char buffer[SERVER_BUFFER_CAPACITY];
      memset(buffer, '\0', SERVER_BUFFER_CAPACITY);

      while (client.connected() && cursor < SERVER_BUFFER_CAPACITY - 1 &&
             noreads < MAX_CLIENT_BLANK_READS &&
             (method == Configuring::ERequestParsingMode::None
                  ? cursor < MAX_HEADER_SIZE
                  : true)) {
        // If there is no pending data in our buffer, increment our noop count
        // and move on. If that count exceeds a threshold, we will stop reading.
        if (!client.available()) {
          noreads += 1;
          continue;
        }

        noreads = 0;

        char token = client.read();
        buffer[cursor] = token;
        cursor += 1;

        // TODO: what is this doing?
        if (cursor < 3 || method == Configuring::ERequestParsingMode::Done) {
          continue;
        }

        // If we have not started to parse any response, and the client received
        // a get request to the connect endpoint, we're going to want to send
        // the capture portal html data.
        if (method == Configuring::ERequestParsingMode::None &&
            strcmp(buffer, CONNECTION_PREFIX) == 0) {
          method = Configuring::ERequestParsingMode::StartNetwork;
          field = cursor;
          continue;
        }
      }

      if (method == Configuring::ERequestParsingMode::StartNetwork) {
        log_i("attempting to parse url parameters starting at %d (of %d)",
              field, cursor);

        bool terminating = false;
        uint8_t field_start = field;

        for (uint16_t start = field;
             start < cursor &&
             method != Configuring::ERequestParsingMode::Failed;
             start++) {
          if (terminating && buffer[start] == '\n') {
            break;
          }

          if (buffer[start] == '\r') {
            terminating = true;
            continue;
          }

          if (buffer[start] == '=' &&
              method == Configuring::ERequestParsingMode::StartNetwork) {
            field_start = start + 1;
            method = Configuring::ERequestParsingMode::NetworkValue;
            continue;
          }

          if (buffer[start] == '&' &&
              method == Configuring::ERequestParsingMode::NetworkValue) {
            uint16_t len = start - field_start;

            if (len < MAX_NETWORK_CREDENTIAL_SIZE) {
              fields_set += 1;
              memcpy(_ssid.get(), buffer + field_start, len);
              log_i("terminated SSID name value parsing: %d",
                    strlen((char *)_ssid.get()));

              // HACK: url decoding
              for (uint16_t i = 0; i < len; i++) {
                if (_ssid->at(i) == '+') {
                  std::array<char, MAX_NETWORK_CREDENTIAL_SIZE> *mem =
                      _ssid.get();
                  mem->at(i) = ' ';
                  // _ssid.get()[i] = ' ';
                }
              }

              method = Configuring::ERequestParsingMode::PasswordStart;
            } else {
              log_e("parsed ssid name too long: %d", len);
              method = Configuring::ERequestParsingMode::Failed;
            }

            continue;
          }

          if (buffer[start] == '=' &&
              method == Configuring::ERequestParsingMode::PasswordStart) {
            field_start = start + 1;
            method = Configuring::ERequestParsingMode::PasswordValue;
            continue;
          }

          if (buffer[start] == ' ' &&
              method == Configuring::ERequestParsingMode::PasswordValue) {
            uint16_t len = start - field_start;

            if (len < MAX_NETWORK_CREDENTIAL_SIZE) {
              fields_set += 1;
              memcpy(_password.get(), buffer + field_start, len);
              log_i("terminated SSID password value parsing: %d",
                    strlen((char *)_password.get()));
            } else {
              log_e("parsed ssid password too long: %d", len);
              method = Configuring::ERequestParsingMode::Failed;
            }

            // method = Configuring::ERequestParsingMode::Done;
            continue;
          }
        }
      }

      // For now, always respond to clients with the same html response.
      client.write(index_html, index_end - index_html);
      delay(10);
      client.stop();

      if (fields_set == 2) {
        log_i("wifi credentials ready ('%s' '%s')", _ssid.get(),
              _password.get());

        log_i("explicitly stopping ESP wifi server");
        configuring._server->stop();
        configuring._server = NULL;

        log_i("performing ESP wifi disconnect");
        WiFi.softAPdisconnect(true);
        WiFi.disconnect(true, true);
        log_i("successfully shut down wifi access point");

        return std::make_tuple(Connecting{}, EMessage::AttemptingConnection);
      }

      // If we finished reading all the data available and we're not done, this
      // is where we will write the html data.
      log_e("non-connect request after %d bytes:\n%s", cursor, buffer);

      return std::make_tuple(State(std::move(configuring)), initial);
    }

    std::tuple<State, std::optional<EMessage>> operator()(
        Connecting &&connecting) {
      if (connecting.attempt == 0) {
        log_i("wifi attempting (ssid: %d, password: %d)",
              strlen((char *)_ssid.get()), strlen((char *)_password.get()));

        WiFi.mode(WIFI_STA);

        int network_count = WiFi.scanNetworks();
        log_i("found %d networks", network_count);
        for (int i = 0; i < network_count; ++i) {
          auto ssid = WiFi.SSID(i);
          char ssid_name[256] = {'\0'};
          ssid.toCharArray(ssid_name, 256);
          log_i("network: %s", ssid_name);
        }

        WiFi.config(INADDR_NONE, INADDR_NONE, INADDR_NONE, INADDR_NONE);
        WiFi.setHostname("orient-beetle");
        WiFi.begin(_ssid->data(), _password->data());
      }

      if (WiFi.status() == WL_CONNECTED) {
        log_i("wifi is connected");
        _preferences->putString("ssid", (char *)_ssid->data());
        _preferences->putString("password", (char *)_password->data());
        return std::make_tuple(Active{true}, EMessage::Connected);
      }

      // TODO: currently this is just time based. It would be better to hook
      // into the ESP32 wifi libraries directly.
      if (_time - _last_connecting_inc > 500) {
        log_i("wifi events incrementing pending connection attempt %d",
              connecting.attempt);
        _last_connecting_inc = _time;
        connecting.attempt += 1;
      }

      if (connecting.attempt > MAX_PENDING_CONNECTION_ATTEMPTS) {
        WiFi.disconnect(true, true);
        return std::make_tuple(Configuring{}, EMessage::FailedConnection);
      }

      return std::make_tuple(connecting, std::nullopt);
    }

    std::tuple<State, std::optional<EMessage>> operator()(Active &&active) {
      uint8_t still_connected = WiFi.status() == WL_CONNECTED ? 1 : 0;

      if (_time - _last_debug > 3000) {
        _last_debug = _time;

        if (still_connected) {
          IPAddress address = WiFi.localIP();
          auto addr = address.toString();
          char addr_str[256] = {'\0'};
          addr.toCharArray(addr_str, 256);
          log_i("wifi events still active: (%s)", addr_str);
        }
      }

      if (still_connected == 0 && active.ok) {
        active.ok = false;
        log_e("wifi connection interrupted");
        return std::make_tuple(active, EMessage::ConnectionInterruption);
      }

      if (still_connected == 1 && !active.ok) {
        active.ok = true;
        active.interrupts = 0;
        log_i("wifi connection recovered after %d", active.interrupts);
        return std::make_tuple(active, EMessage::ConnectionResumed);
      }

      if (still_connected == 0 && !active.ok &&
          _time - _last_connecting_inc > 100) {
        _last_connecting_inc = _time;
        active.interrupts += 1;
        log_i("wifi connection still interrupted after: %d", active.interrupts);
      }

      if (active.interrupts > MAX_PENDING_CONNECTION_ATTEMPTS) {
        log_e("wifi connection being destroyed after: %d", active.interrupts);

        _preferences->remove("ssid");
        _preferences->remove("password");

        return std::make_tuple(Configuring{}, EMessage::Disconnected);
      }

      return std::make_tuple(active, std::nullopt);
    }

    const std::tuple<const char *, const char *> *_ap_config;
    uint32_t _time;
    std::shared_ptr<std::array<char, MAX_NETWORK_CREDENTIAL_SIZE>> _ssid;
    std::shared_ptr<std::array<char, MAX_NETWORK_CREDENTIAL_SIZE>> _password;
    std::shared_ptr<Preferences> _preferences;
    uint32_t _last_debug = 0;
    uint32_t _last_connecting_inc = 0;
  };

  State _mode;
  std::tuple<const char *, const char *> _ap_config;
  std::shared_ptr<std::array<char, MAX_NETWORK_CREDENTIAL_SIZE>> _ssid;
  std::shared_ptr<std::array<char, MAX_NETWORK_CREDENTIAL_SIZE>> _password;
  std::shared_ptr<Preferences> _preferences;
  Visitor _visitor;
  uint32_t _last_time = 0;
};

}  // namespace wifievents


#include "wifi-manager.hpp"

namespace wifimanager {
  Manager::Manager(std::tuple<const char *, const char *> ap):
    _last_mode(0),
    _ap_config(ap),
    _mode(false)
  {
    _mode.emplace<PendingConfiguration>();
  }

  inline uint8_t Manager::attempt(void) {
    if (_mode.index() == 2) {
      return std::get_if<PendingConnection>(&_mode)->_attempts;
    }
    return 0;
  }

  std::optional<Manager::EManagerMessage> Manager::frame() {
    unsigned int modi = _mode.index();

#ifndef RELEASE
    if (_last_mode != modi) {
      switch (modi) {
        case 0:
          log_d("active");
          break;
        case 1:
          log_d("waiting for configration");
          break;
        case 2:
          log_d("connecting to network");
          break;
      }

      _last_mode = modi;
    }
#endif

    switch (modi) {
      case 0: {
        ActiveConnection * active = std::get_if<0>(&_mode);
        uint8_t previous = active->_disconnected;

        active->_disconnected = WiFi.status() == WL_CONNECTED
          ? 0
          : active->_disconnected + 1;

        // If we're now disconnected, sent an interruption message.
        if (active->_disconnected == 1) {
          return Manager::EManagerMessage::ConnectionInterruption;
        }

        // If we're no longer disconnected, but were previously, we've been resumed.
        if (active->_disconnected == 0 && previous != 0) {
          return Manager::EManagerMessage::ConnectionResumed;
        }

        if (active->_disconnected > MAX_CONNECTION_INTERRUPTS) {
          log_e("wifi manager disonncted after %d attempts", active->_disconnected);

          _mode.emplace<PendingConfiguration>();
          return Manager::EManagerMessage::Disconnected;
        }

        break;
      }

      /**
       * Configuration Mode
       *
       * When the `_mode` variant is a `WiFiServer`, we are waiting for someone to
       * load the index html page and enter in the wifi credentials.
       */
      case 1: {
        PendingConfiguration * server = std::get_if<1>(&_mode);

        // TODO: using stack allocated char arrays of a preset max size here over
        // dynamically allocated memory. Not clear right now which is better.
        char ssid [MAX_SSID_LENGTH];
        memset(ssid, '\0', MAX_SSID_LENGTH);
        char password [MAX_PASSWORD_LENGTH];
        memset(password, '\0', MAX_PASSWORD_LENGTH);

        if (server->frame(ssid, password) == false) { 
          break;
        }

        // Terminate our hotspot, we have everything we need to make an attempt to
        // establish a connection with the wifi network.
        WiFi.softAPdisconnect(true);
        WiFi.disconnect(true, true);

        // Move ourselves into the pending connection state. This will terminate our
        // wifi server.
        _mode.emplace<PendingConnection>(ssid, password);

        WiFi.mode(WIFI_STA);
        return Manager::EManagerMessage::Connecting;
      }

      /**
       * Connection Mode
       *
       * During this phase, we have an ssid + password ready, we just need to attempt
       * to boot the wifi module and wait for it to be connected.
       */
      case 2: {
        PendingConnection * pending = std::get_if<2>(&_mode);

        if (pending->_attempts % 3 == 0) {
          log_d("attempting to connect to wifi [%d]", pending->_attempts);
        }

        if (pending->_attempts == 0) {
          log_d("connecting to wifi");
          WiFi.begin(pending->_ssid, pending->_password);
        }

        // If we have a connection, move out of this mode
        if (WiFi.status() == WL_CONNECTED) {
          log_d("wifi is connected");
          _mode.emplace<ActiveConnection>(0);
          return Manager::EManagerMessage::Connected;
        }

        pending->_attempts += 1;

        // If we have seen too many frames without establishing a connection to the 
        // network provided by the user, move back into the AP/configuration mode.
        if (pending->_attempts > MAX_PENDING_CONNECTION_ATTEMPTS) {
          log_d("too many connections failed, resetting");

          // Clear out our connection attempt garbage.
          WiFi.disconnect(true, true);

          // Prepare the wifi server.
          _mode.emplace<PendingConfiguration>();

          // Enter into AP mode and start the server.
          begin();
          return Manager::EManagerMessage::FailedConnection;
        }

        break;
      }
      default:
        log_d("unknown state");
        break;
    }

    return std::nullopt;
  }

  void Manager::begin(void) {
    if (_mode.index() == 1) {
      WiFi.softAP(std::get<0>(_ap_config), std::get<1>(_ap_config), 7, 0, 1);
      IPAddress address = WiFi.softAPIP();

      log_d("AP IP address: %s", address.toString());
      std::get_if<1>(&_mode)->begin(address);
      return;
    }

    log_d("soft ap not started");
  }

  bool Manager::PendingConfiguration::frame(char * ssid, char * password) {
      WiFiClient client = available();

      // If we are running in AP mode and have no http connection to our server, move right along.
      if (!client) {
        return false;
      }

      // TODO: figure out how to decouple this so that consumers can provide their own index html.
      // Currently, this is used to avoid the RAM cost associated with carrying around the char[]
      extern const char index_html[] asm("_binary_embeds_index_http_start");
      extern const char index_end[] asm("_binary_embeds_index_http_end");

      log_d("loaded index (%d bytes)", index_end - index_html);

      unsigned int cursor = 0, field = 0;
      unsigned char noreads = 0;

      ERequestParsingMode method = ERequestParsingMode::None;

      // stack-allocated space with immediate initialization?
      char buffer [SERVER_BUFFER_CAPACITY];
      memset(buffer, '\0', SERVER_BUFFER_CAPACITY);

      while (
        client.connected()
          && cursor < SERVER_BUFFER_CAPACITY - 1
          && noreads < MAX_CLIENT_BLANK_READS
          && (method != ERequestParsingMode::None ? true : cursor < MAX_HEADER_SIZE)
      ) {
        // If there is no pending data in our buffer, increment our noop count and
        // move on. If that count exceeds a threshold, we will stop reading.
        if (!client.available()) {
          noreads += 1;
          delay(10);
          continue;
        }

        // Reset our message-less count.
        noreads = 0;

        // Pull the next character off our client.
        buffer[cursor] = client.read();

        if (cursor < 3 || method == ERequestParsingMode::Done) {
          cursor += 1;
          continue;
        }

        if (method == ERequestParsingMode::None && strcmp(buffer, CONNECTION_PREFIX) == 0) {
          log_d("found connection request, preparing for ssid parsing");

          method = ERequestParsingMode::Network;
          cursor += 1;
          field = cursor;
          continue;
        }

        if (PendingConfiguration::termination(method) == buffer[cursor]) {
          unsigned char offset = 0;
          const char * value = buffer + offset + field;

          while ((offset + field) < cursor && *value != '=') {
            value = buffer + offset + field;
            offset += 1;
          }

          switch (method) {
            case ERequestParsingMode::Network:
              method = ERequestParsingMode::Password;
              memcpy(ssid, buffer + field + offset, cursor - (field + offset));
              break;
            case ERequestParsingMode::Password:
              method = ERequestParsingMode::Done;
              memcpy(password, buffer + field + offset, cursor - (field + offset));
              break;
            default:
              break;
          }

          cursor += 1;
          field = cursor;
          continue;
        }

        cursor += 1;
      }

      if (method != ERequestParsingMode::Done) {
        log_d("non-connect request:\n%s", buffer);

        client.write(index_html, index_end - index_html);
        delay(10);
        client.stop();
        return false;
      }

      log_d("[wifi_manager] ssid: %s | password %s", ssid, password);

      client.write(index_html, index_end - index_html);
      delay(10);
      client.stop();

      return true;
  }

  /**
   * When parsing the statusline of a request, this function will return the character
   * that is expected to terminate a given parsing mode.
   */
  inline char Manager::PendingConfiguration::termination(ERequestParsingMode mode) {
    switch (mode) {
      case ERequestParsingMode::Network:
        return '&';
      case ERequestParsingMode::Password:
        return ' ';
      default:
        return '\0';
    }
  }

  void Manager::PendingConfiguration::begin(IPAddress addr) {
    _server.begin();
    _dns.start(53, "*", addr);
  }

  WiFiClient Manager::PendingConfiguration::available(void) {
    _dns.processNextRequest();
    return _server.available();
  }

  Manager::PendingConfiguration::~PendingConfiguration() {
    log_d("wifi_manager::pending_configuration", "exiting pending configuration");

    _server.stop();
    _dns.stop();
  }
}

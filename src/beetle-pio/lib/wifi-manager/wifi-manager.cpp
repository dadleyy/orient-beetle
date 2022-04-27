#include "wifi-manager.hpp"

namespace wifimanager {
  Manager::Manager(const char * index, std::tuple<const char *, const char *> ap):
    _last_frame(0),
    _last_mode(0),
    _index(index),
    _ap_config(ap),
    _mode(false)
  {
    _mode.emplace<PendingConfiguration>(_index);
  }

  Manager::~Manager() {
  }

  bool Manager::ready(void) {
    if (_mode.index() == 0) {
      bool connected = *std::get_if<0>(&_mode);
      return connected;
    }

    return false;
  }

  void Manager::frame(unsigned long now) {
    unsigned int modi = _mode.index();

    if (now - _last_frame < MIN_FRAME_DELAY) {
      return;
    }

#ifndef RELEASE
    if (_last_mode != modi) {
      switch (modi) {
        case 0:
          Serial.println("[wifi_manager]: active");
          break;
        case 1:
          Serial.println("[wifi_manager]: waiting for user configuration");
          break;
        case 2:
          Serial.println("[wifi_manager]: waiting for connection to network");
          break;
      }

      _last_mode = modi;
    }
#endif

    _last_frame = now;

    switch (modi) {
      case 0: {
        // Continue to verify our wifi connection status.
        _mode.emplace<bool>(WiFi.status() == WL_CONNECTED);
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
        char ssid [MAX_SSID_LENGTH] = {'\0'};
        char password [MAX_SSID_LENGTH] = {'\0'};

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
        break;
      }

      /**
       * Connection Mode
       *
       * During this phase, we have an ssid + password ready, we just need to attempt
       * to boot the wifi module and wait for it to be connected.
       */
      case 2: {
        PendingConnection * pending = std::get_if<2>(&_mode);

#ifndef RELEASE
        Serial.print("[wifi_manager] attempting to connect to wifi [");
        Serial.print(pending->_attempts);
        Serial.println("]");
#endif

        if (pending->_attempts == 0) {
#ifndef RELEASE
          Serial.print("connecting to wifi");
#endif
          WiFi.begin(pending->_ssid, pending->_password);
        }

        // If we have a connection, move out of this mode
        if (WiFi.status() == WL_CONNECTED) {
#ifndef RELEASE
          Serial.println("[wifi_manager] wifi connection established, moving mode to connected");
#endif

          _mode.emplace<bool>(true);
          break;
        }

        pending->_attempts += 1;

        // If we have seen too many frames without establishing a connection to the 
        // network provided by the user, move back into the AP/configuration mode.
        if (pending->_attempts > MAX_PENDING_CONNECTION_ATTEMPTS) {
#ifndef RELEASE
          Serial.println("[wifi_manager] too many pending connection attempts, moving back to ap");
#endif

          // Clear out our connection attempt garbage.
          WiFi.disconnect(true, true);

          // Prepare the wifi server.
          _mode.emplace<PendingConfiguration>(_index);

          // Enter into AP mode and start the server.
          begin();
        }
        break;
      }
      default:
#ifndef RELEASE
        Serial.println("[wifi_manager] wifi manager: unknown mode");
#endif
        break;
    }
  }

  void Manager::begin(void) {
    if (_mode.index() == 1) {
      WiFi.softAP(std::get<0>(_ap_config), std::get<1>(_ap_config), 7, 0, 1);
      IPAddress address = WiFi.softAPIP();

#ifndef RELEASE
      Serial.print("[wifi_manager] AP IP address: ");
      Serial.println(address);
      Serial.println("--- boot complete ---");
#endif

      std::get_if<1>(&_mode)->begin(address);
      return;
    }

#ifndef RELEASE
    Serial.println("[wifi_manager] soft ap not started");
#endif
  }

  bool Manager::PendingConfiguration::frame(char * ssid, char * password) {
      WiFiClient client = available();

      // If we are running in AP mode and have no http connection to our server,
      // move right along.
      if (!client) {
        return false;
      }

      unsigned int cursor = 0, field = 0;
      unsigned char noreads = 0;

      ERequestParsingMode method = ERequestParsingMode::None;
      char buffer [SERVER_BUFFER_CAPACITY] = {'\0'};

      memset(buffer, '\0', SERVER_BUFFER_CAPACITY);
      memset(ssid, '\0', MAX_SSID_LENGTH);
      memset(password, '\0', MAX_SSID_LENGTH);

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
#ifndef RELEASE
          Serial.println("[wifi_manager] found connection request, preparing for ssid parsing");
#endif

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
#ifndef RELEASE
        Serial.print("[wifi_manager] non-connect request:\n");
        Serial.println(buffer);
#endif

        client.println(_index);
        client.stop();
        return false;
      }

#ifndef RELEASE
      Serial.print("[wifi_manager] ssid: '");
      Serial.print(ssid);
      Serial.println("'");
      Serial.print("[wifi_manager] password: '");
      Serial.print(password);
      Serial.println("'");
#endif

      client.println(_index);
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

  WiFiClient Manager::PendingConfiguration::available(void) {
    _dns.processNextRequest();
    return _server.available();
  }
}

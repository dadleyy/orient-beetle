#include "wifi-manager.hpp"

namespace wifimanager {
  Manager::Manager(const char * index, std::tuple<const char *, const char *> ap):
    _last_frame(0),
    _index(index),
    _ap_config(ap),
    _mode(false)
  {
    _mode.emplace<WiFiServer>(80);
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

    _last_frame = now;

    switch (modi) {
      case 0: {
#ifndef RELEASE
        if (ready()) {
          Serial.println("wifi manager: connected");
        } else {
          Serial.println("wifi manager: not connected");
        }
#endif
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
        Serial.println("wifi manager: configuration");
        WiFiServer * server = std::get_if<1>(&_mode);
        WiFiClient client = server->available();

        // If we are running in AP mode and have no http connection to our server,
        // move right along.
        if (!client) {
          return;
        }

        unsigned int cursor = 0;
        unsigned char noreads = 0;
        bool head = false;
        char buffer [SERVER_BUFFER_CAPACITY] = {'\0'};
        memset(buffer, '\0', SERVER_BUFFER_CAPACITY);

        while (
          client.connected()
            && cursor < SERVER_BUFFER_CAPACITY - 1
            && noreads < MAX_CLIENT_BLANK_READS
            && (head ? true : cursor < MAX_HEADER_SIZE)
        ) {
          // If there is no pending data in our buffer, increment our noop count and
          // move on. If that count exceeds a threshold, we will stop reading.
          if (!client.available()) {
            noreads += 1;
            delay(50);
            continue;
          }

          noreads = 0;
          char c = client.read();
          buffer[cursor] = c;

          // If we have read more than 3 characters we are ready to start trying to
          // determine whether or not we have reached the end of our headers.
          if (cursor >= 3 && head == false) {
            char header [5] = {'\0'};

            for (unsigned char i = 0; i < 4; i++) {
              header[i] = buffer[cursor - (3 - i)];
            }

            // If we have found the end of the HTTP headers, clear out our buffer to
            // prepare for body data and reset our cursor.
            if (strcmp(header, HEADER_DELIM) == 0) {
              memset(buffer, '\0', SERVER_BUFFER_CAPACITY);
              head = true;
              cursor = 0;
              continue;
            }
          }

          cursor += 1;
        }

        // The initial `GET` request to our server running in AP mode will not have 
        // any body. This is the request to respond with the html.
        if (strlen(buffer) == 0) {
#ifndef RELEASE
          Serial.println("responding with index");
#endif

          client.println(_index);
          client.stop();
          return;
        }

#ifndef RELEASE
        Serial.println("had body - using for ssid/password");
        Serial.println(buffer);
#endif

        char ssid [MAX_SSID_LENGTH] = {'\0'};
        char password [MAX_PASSWORD_LENGTH] = {'\0'};
        unsigned int point = 0;
        bool stage = false;
        bool failed = false;

        // Attempt to parse our form data into individual ssid + password buffers.
        for (unsigned int i = 0; i < cursor; i++) {
          // TODO: Anyime we encounter an unescaped '&' we're going to assume that what follows should be parsed as a
          // password. This relies on the ssid being sent before the password.
          if (buffer[i] == '&' & !stage) {
            stage = true;
            point = 0;
            memset(password, MAX_PASSWORD_LENGTH, '\0');
            continue;
          }

          // If we have not moved into parsing the password.
          if (!stage) {
            if (point >= MAX_SSID_LENGTH) {
              failed = true;
              break;
            }

            ssid[point] = buffer[i];
            point += 1;

            if (strcmp(ssid, "ssid=") == 0) {
              memset(ssid, MAX_SSID_LENGTH, '\0');
              point = 0;
              continue;
            }

            continue;
          }

          // Protect against a user-submitted value that is too long to be a password.
          if (point >= MAX_PASSWORD_LENGTH) {
            failed = true;
            break;
          }

          password[point] = buffer[i];
          point += 1;

          if (strcmp(password, "password=") == 0) {
            memset(ssid, MAX_PASSWORD_LENGTH, '\0');
            point = 0;
            continue;
          }
        }

#ifndef RELEASE
        Serial.print("ssid: ");
        Serial.println(ssid);
        Serial.print("password: ");
        Serial.println(password);
#endif

        if (failed || strlen(ssid) == 0) {
#ifndef RELEASE
          Serial.println("[warning] failed user input parsing");
#endif
          client.println(F("HTTP/1.1 301 Redirect\r\nLocation: http://192.168.4.1\r\n\r\n"));
          client.stop();

          return;
        } else {
          // Respond with a redirect to avoid the double-form refresh issue.
          client.println(CONFIG_REDIRECT);
        }

        client.stop();

        // Terminate our hotspot, we have everything we need to make an attempt to
        // establish a connection with the wifi network.
        WiFi.softAPdisconnect(true);

        // Move ourselves into the pending connection state. This will terminate our
        // wifi server.
        _mode.emplace<PendingConnection>(ssid, password);
        break;
      }

      /**
       * Connection Mode
       *
       * During this phase, we have an ssid + password ready, we just need to attempt
       * to boot the wifi module and wait for it to be connected.
       */
      case 2: {
#ifndef RELEASE
        Serial.println("wifi manager: pending connection");
#endif
        PendingConnection * pending = std::get_if<2>(&_mode);

#ifndef RELEASE
        Serial.print("attempting to connect to wifi [");
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
          Serial.println("wifi connection established, moving mode to connected");
#endif

          _mode.emplace<bool>(true);
          break;
        }

        pending->_attempts += 1;

        // If we have seen too many frames without establishing a connection to the 
        // network provided by the user, move back into the AP/configuration mode.
        if (pending->_attempts > MAX_PENDING_CONNECTION_ATTEMPTS) {
#ifndef RELEASE
          Serial.println("too many pending connection attempts, moving back to ap");
#endif

          // Clear out our connection attempt garbage.
          WiFi.disconnect(true, true);

          // Prepare the wifi server.
          _mode.emplace<WiFiServer>(80);

          // Enter into AP mode and start the server.
          begin();
        }
        break;
      }
      default:
#ifndef RELEASE
        Serial.println("wifi manager: unknown mode");
#endif
        break;
    }
  }

  void Manager::begin(void) {
    if (_mode.index() == 1) {
      WiFi.softAP(std::get<0>(_ap_config), std::get<1>(_ap_config), 7, 0, 1);

#ifndef RELEASE
      IPAddress IP = WiFi.softAPIP();
      Serial.print("AP IP address: ");
      Serial.println(IP);
      Serial.println("--- boot complete ---");
#endif

      std::get_if<1>(&_mode)->begin();
      return;
    }

#ifndef RELEASE
    Serial.println("soft ap not started");
#endif
  }
}

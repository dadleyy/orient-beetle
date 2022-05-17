#ifndef _WIFI_MANAGER_H
#define _WIFI_MANAGER_H 1

#include <variant>
#include <optional>
#include "esp32-hal-log.h"

#include <WiFi.h>
#include <DNSServer.h>
#include <WiFiClient.h>
#include <WiFiServer.h>
#include <WiFiAP.h>

namespace wifimanager {

  class Manager final {
    public:
      explicit Manager(std::tuple<const char *, const char *>);
      ~Manager() = default;

      // Disable Copy
      Manager(const Manager &) = delete;
      Manager & operator=(const Manager &) = delete;

      // Disable Move
      Manager(Manager &&) = delete;
      Manager& operator=(Manager &&) = delete;

      enum EManagerMessage {
        Connecting = 0,
        Connected,
        FailedConnection,
        Disconnected,
        ConnectionInterruption,
        ConnectionResumed,
      };

      void begin(void);
      std::optional<EManagerMessage> frame();
      uint8_t attempt(void);

    private:
      constexpr static const char * CONNECTION_PREFIX = "GET /connect?";
      constexpr static uint16_t SERVER_BUFFER_CAPACITY = 1024;
      constexpr static uint8_t MAX_CLIENT_BLANK_READS = 5;
      constexpr static uint16_t MAX_PENDING_CONNECTION_ATTEMPTS = 200;
      constexpr static uint16_t MAX_CONNECTION_INTERRUPTS = 500;
      constexpr static uint16_t MAX_HEADER_SIZE = 512;

      constexpr static uint8_t MAX_SSID_LENGTH = 60;
      constexpr static uint8_t MAX_PASSWORD_LENGTH  = 30;

      enum ERequestParsingMode {
        None = 0,
        Network = 1,
        Password = 2,
        Done = 3,
        Failed = 4,
      };

      // Initially, we do not have the necessary information to connect to a
      // wifi network. While in this state, we will run both an http server
      // as well as a dns server to create a "captive portal"
      struct PendingConfiguration final {
        public:
          PendingConfiguration(): _server(80) {}
          ~PendingConfiguration();

          PendingConfiguration(const PendingConfiguration&) = delete;
          PendingConfiguration & operator=(const PendingConfiguration&) = delete;

          bool frame(char *, char *);
          void begin(IPAddress addr);

        private:
          WiFiClient available(void);
          inline static char termination(ERequestParsingMode);

          WiFiServer _server;
          DNSServer _dns;
      };

      // Once the user submits their wifi network configuration settings, we'll
      // attempt to connect via `WiFi.begin(...)` and wait a defined number of
      // frames before aborting back to configuration.
      struct PendingConnection final {
        uint8_t _attempts = 0;

        // TODO: unsure if using pointers here vs arrays with constant sizes is
        // more "proper". Since we're dealing with a small amount (max 60 + 40
        // bytes) of data, it might be easier to use array members.
        char * _ssid;
        char * _password;

        PendingConnection(const char * ssid, const char * password):
          _attempts(0),
          _ssid((char *) malloc(sizeof(char) * MAX_SSID_LENGTH)),
          _password((char *) malloc(sizeof(char) * MAX_PASSWORD_LENGTH))
        {
          memcpy(_ssid, ssid, MAX_SSID_LENGTH);
          memcpy(_password, password, MAX_PASSWORD_LENGTH);
        }

        ~PendingConnection() {
          log_d("[MEMORY OPERATION] freeing memory used by pending connection");
          free(_ssid);
          free(_password);
        }

        PendingConnection(const PendingConnection &) = delete;
        PendingConnection& operator=(const PendingConnection &) = delete;
      };


      // After we're connected via `WiFi.status(...)` returns a connected state,
      // we'll move into this active connection state where each frame checks
      // the current connection information and disconnects after some number of
      // frames.
      struct ActiveConnection final {
        explicit ActiveConnection(uint8_t d): _disconnected(d) {}
        uint8_t _disconnected = 0;
      };

      uint8_t _last_mode;
      std::tuple<const char *, const char *> _ap_config;
      std::variant<ActiveConnection, PendingConfiguration, PendingConnection> _mode;
  };
}

#endif

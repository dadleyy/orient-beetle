#ifndef _WIFI_MANAGER_H
#define _WIFI_MANAGER_H 1

#include <variant>
#include <optional>

#include <WiFi.h>
#include <DNSServer.h>
#include <WiFiClient.h>
#include <WiFiServer.h>
#include <WiFiAP.h>

namespace wifimanager {

  struct Manager final {
    constexpr static const char * CONNECTION_PREFIX = "GET /connect?";

    constexpr static unsigned int SERVER_BUFFER_CAPACITY = 1024;
    constexpr static unsigned char MAX_CLIENT_BLANK_READS = 5;
    constexpr static unsigned char MAX_PENDING_CONNECTION_ATTEMPTS = 200;
    constexpr static unsigned int MAX_HEADER_SIZE = 512;
    constexpr static unsigned char MAX_SSID_LENGTH = 60;
    constexpr static unsigned char MAX_PASSWORD_LENGTH  = 30;

    Manager(std::tuple<const char *, const char *>);
    ~Manager() = default;

    enum EManagerMessage {
      Connecting,
      Connected,
      FailedConnection,
      Disconnected,
      ConnectionInterruption,
      ConnectionResumed,
    };

    void begin(void);
    std::optional<EManagerMessage> frame();

    private:
      // It is not clear now what copy move and move assignment look like. Disable for now.
      Manager(const Manager &) = default;
      Manager(Manager &&) = default;
      Manager & operator=(const Manager &) = default;

      unsigned char _last_mode;
      std::tuple<const char *, const char *> _ap_config;

      enum ERequestParsingMode {
        None = 0,
        Network = 1,
        Password = 2,
        Done = 3,
        Failed = 4,
      };

      /**
       * `PendingConfiguration` represents the initial "resting" state of the device. During
       * this state, we are running an http server _and_ a dns server. As devices connect to
       * the network,
       */
      struct PendingConfiguration {
        public:
          PendingConfiguration(): _server(80) {}
          ~PendingConfiguration();

          bool frame(char *, char *);
          void begin(IPAddress addr);

        private:
          WiFiClient available(void);
          inline static char termination(ERequestParsingMode);

          WiFiServer _server;
          DNSServer _dns;
      };

      /**
       * The `PendingConnection` variant is used as the state immediately after a user has
       * submitted the network ssid and password.
       */
      struct PendingConnection {
        uint8_t _attempts = 0;
        char _ssid [MAX_SSID_LENGTH] = {'\0'};
        char _password [MAX_PASSWORD_LENGTH] = {'\0'};

        PendingConnection(
            char ssid [MAX_SSID_LENGTH],
            char password [MAX_PASSWORD_LENGTH]
        ): _attempts(0) {
          memcpy(_ssid, ssid, MAX_SSID_LENGTH);
          memcpy(_password, password, MAX_PASSWORD_LENGTH);
        }
      };

      struct ActiveConnection {
        ActiveConnection(uint8_t d): _disconnected(d) {}
        uint8_t _disconnected = 0;
      };

      // Note: the additional `bool` at the start of this variant type helps ensure
      // that the constructor of our WiFiServer is called explicitly when we want to.
      // 
      // During this class's constructor, the variant is `.emplace`-ed immediately with
      // a wifi server.
      std::variant<ActiveConnection, PendingConfiguration, PendingConnection> _mode;
  };
}

#endif

#ifndef _WIFI_MANAGER_H
#define _WIFI_MANAGER_H 1

#include <variant>

#include <WiFi.h>
#include <WiFiClient.h>
#include <WiFiAP.h>

namespace wifimanager {

  struct Manager final {
    constexpr static const char * HEADER_DELIM PROGMEM = "\r\n\r\n";
    constexpr static const char * CONFIG_REDIRECT PROGMEM = "HTTP/1.1 301 Redirect\r\nLocation: https://google.com\r\n\r\n";
    constexpr static unsigned int SERVER_BUFFER_CAPACITY = 1024;
    constexpr static unsigned char MAX_CLIENT_BLANK_READS = 5;
    constexpr static unsigned char MAX_PENDING_CONNECTION_ATTEMPTS = 60;
    constexpr static unsigned char MIN_FRAME_DELAY = 100;
    constexpr static unsigned int MAX_HEADER_SIZE = 512;
    constexpr static unsigned char MAX_SSID_LENGTH = 60;
    constexpr static unsigned char MAX_PASSWORD_LENGTH  = 30;

    Manager(const char *, std::tuple<const char *, const char *>);
    ~Manager();

    void begin(void);
    void frame(unsigned long);
    bool ready(void);

    private:
      // It is not clear now what copy move and move assignment look like. Better to
      // prevent them for the time being.
      Manager(const Manager &) = default;
      Manager(Manager &&) = default;
      Manager & operator=(const Manager &) = default;

      unsigned long _last_frame;
      const char * _index;
      std::tuple<const char *, const char *> _ap_config;

      struct PendingConnection {
        unsigned char _attempts = 0;
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

      // Note: the additional `bool` at the start of this variant type helps ensure
      // that the constructor of our WiFiServer is called explicitly when we want to.
      // 
      // During this class's constructor, the variant is `.emplace`-ed immediately with
      // a wifi server.
      std::variant<bool, WiFiServer, PendingConnection> _mode;
  };
}

#endif

#ifndef _REDIS_MANAGER_H
#define _REDIS_MANAGER_H 1

#include <variant>
#include <optional>
#include <WiFiClientSecure.h>

#include "wifi-manager.hpp"

namespace redismanager {
  
  class Manager final {
    public:
      Manager(const char *, const uint32_t, const char *);
      ~Manager() = default;

      enum EManagerMessage {
        FailedConnection,
        ConnectionLost,
        EstablishedConnection,
        ReceivedMessage,
      };

      std::optional<EManagerMessage> frame(std::optional<wifimanager::Manager::EManagerMessage> &message);
      uint16_t copy(char *, uint16_t);

    private:
      // It is not clear now what copy move and move assignment look like. Disable for now.
      Manager(const Manager &) = default;
      Manager(Manager &&) = default;
      Manager & operator=(const Manager &) = default;

      constexpr static const uint32_t framebuffer_size = 1024;
      constexpr static const char redis_pop [] = "*2\r\n$4\r\nLPOP\r\n$4\r\nob:m\r\n";

      enum ECertificationStage {
        NotRequested,
        CerificationRequested,
        Certified,
      };

      struct Connected {
        ECertificationStage certified = ECertificationStage::NotRequested;
        WiFiClientSecure client;
        uint16_t cursor = 0;
        char framebuffer[framebuffer_size];

        uint16_t copy(char *, uint16_t);
        std::optional<EManagerMessage> update(const char *, const char *, uint32_t);
      };

      struct Disconnected {
        uint8_t tick = 0;

        bool update(std::optional<wifimanager::Manager::EManagerMessage> &message);
      };

      const char * _redis_host;
      const uint32_t _redis_port;
      const char * _redis_auth;
      bool _paused;

      std::variant<Disconnected, Connected> _state;
  };

}

#endif

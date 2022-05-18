#ifndef _REDIS_MANAGER_H
#define _REDIS_MANAGER_H 1

#include <variant>
#include <optional>
#include <WiFiClientSecure.h>

#include "wifi-manager.hpp"

namespace redismanager {
  
  class Manager final {
    public:
      explicit Manager(std::tuple<const char *, const uint32_t, const char *>);
      ~Manager() = default;

      // Disable Copy
      Manager(const Manager &) = delete;
      Manager & operator=(const Manager &) = delete;

      // Disable Move
      Manager(Manager &&) = delete;
      Manager& operator=(Manager &&) = delete;

      enum EManagerMessage {
        FailedConnection,
        ConnectionLost,
        EstablishedConnection,
        IdentificationReceived,
        ReceivedMessage,
      };

      std::optional<EManagerMessage> frame(std::optional<wifimanager::Manager::EManagerMessage> &message);
      uint16_t copy(char *, uint16_t);

      uint8_t id_size(void);
      uint8_t copy_id(char *, uint8_t);

    private:
      constexpr static const uint32_t FRAMEBUFFER_SIZE = 1024;
      constexpr static const uint32_t MAX_ID_SIZE = 36;

      // registration queues:
      // - `ob:r` -> device pulls its id down
      // - `ob:i` -> device notifies it is online
      constexpr static const char REDIS_REGISTRATION_POP [] = "*2\r\n$4\r\nLPOP\r\n$4\r\nob:r\r\n";

      enum ECertificationStage {
        NotRequested,             // <- connects + writes auth
        CerificationRequested,    // <- reads `+OK`
        Certified,                // <- writes registrar-pop
        IdentificationRequested,  // <- reads id
        Identified,               // <- reads messages
      };

      // Once our wifi manager has established connection, we will open up a tls-backed tcp
      // connection with our redis host and attempt authentication + "streaming".
      struct Connected final {
        public:
          Connected();
          ~Connected();

          Connected(const Connected &) = delete;
          Connected & operator=(const Connected &) = delete;

          Connected(Connected &&) = delete;

          uint16_t copy(char *, uint16_t);
          std::optional<EManagerMessage> update(const char *, const char *, uint32_t);

        private:
          inline uint16_t write_pop(void);
          inline uint16_t write_push(void);

          ECertificationStage _certified;

          // The `_cursor` represents the last index of our framebuffer we have pushed into.
          uint16_t _cursor;
          char * _framebuffer;

          uint8_t _write_delay;
          char _device_id [MAX_ID_SIZE + 1];
          uint8_t _device_id_len;
          WiFiClientSecure _client;

          friend class Manager;
      };

      // Until our wifi manager is connected, this state represents doing nothing.
      struct Disconnected final {
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

#ifndef _REDIS_MANAGER_H
#define _REDIS_MANAGER_H 1

#include <variant>
#include <optional>
#include <WiFiClientSecure.h>
#include <Preferences.h>

#include "wifi-manager.hpp"

namespace redismanager {
  
  class Manager final {
    public:
      explicit Manager(std::tuple<const char *, const uint32_t, std::pair<const char *, const char *>>);
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

      void begin(void);
      std::optional<EManagerMessage> frame(std::optional<wifimanager::Manager::EManagerMessage> &message);
      uint16_t copy(char *, uint16_t);

      uint8_t id_size(void);
      uint8_t copy_id(char *, uint8_t);

    private:
      constexpr static const uint32_t FRAMEBUFFER_SIZE = 1024;
      constexpr static const uint32_t MAX_ID_SIZE = 36;

      // The amount of attempts to read from our tls connection that return no data
      // before we attempt to reconnect.
      constexpr static const uint32_t MAX_EMPTY_READ_RESET = 100;

      // The amount of times we reset our connection before we will re-request a new device id.
      constexpr static const uint8_t MAX_RESETS_RECREDENTIALIZE = 5;

      constexpr static const char * OK = "+OK\r\n";
      constexpr static const char * WRONG_PASS_ERR = "-WRONGPASS invalid username-password pair or user is disabled\r\n";
      constexpr static const char * NO_PERM_ERR = "-NOPERM this user has no permissions to run the 'rpush' command or its subcommand\r\n";

      // registration queues:
      // - `ob:r` -> device pulls its id down
      // - `ob:i` -> device notifies it is online
      constexpr static const char REDIS_REGISTRATION_POP [] = "*2\r\n$4\r\nLPOP\r\n$4\r\nob:r\r\n";

      enum ECertificationStage {
        NotRequested,             // <- connects + writes auth
        AuthorizationRequested,   // <- reads `+OK`
        AuthorizationReceived,    // <- writes registrar-pop
        IdentificationRequested,  // <- reads id
        AuthorizationAttempted,   // <- waiting for response from `AUTH`
        FullyAuthorized,          // <- reads messages
      };

      // Once our wifi manager has established connection, we will open up a tls-backed tcp
      // connection with our redis host and attempt authentication + "streaming".
      struct Connected final {
        public:
          explicit Connected(Preferences*);
          ~Connected();

          Connected(const Connected &) = delete;
          Connected & operator=(const Connected &) = delete;

          Connected(Connected &&) = delete;

          uint16_t copy(char *, uint16_t);
          std::optional<EManagerMessage> update(
            const char *,
            const std::pair<const char *, const char *>&,
            uint32_t
          );

          void reset(void);

        private:
          inline uint16_t write_push(void);
          inline uint16_t write_pop(void);
          std::optional<EManagerMessage> connect(
            const char *,
            const std::pair<const char *, const char *>&,
            uint32_t
          );

          ECertificationStage _certified;

          // The `_cursor` represents the last index of our framebuffer we have pushed into.
          uint16_t _cursor;

          // This memory is filled every update with the contents of our tcp connection.
          char * _framebuffer;

          // FIXME: This is being used as a poor version of gating the frequency that we will
          // attempt to write pop/push messages. This is somehow related to the `main.cpp`
          // interval.
          uint8_t _write_delay;

          // Credential storage.
          char * _device_id;
          uint8_t _device_id_len;

          // The tcp connection.
          WiFiClientSecure _client;

          // Every time we try to read from our tcp connection that comes back without data,
          // or a response that looks bad, we will increment our count here. If the amount
          // passes the threshold specified by `MAX_EMPTY_READ_RESET`, we will attempt to
          // close and start our tcp connection over.
          uint8_t _empty_identified_reads;
          uint8_t _cached_reset_count;

          // Remember whether or not we connected with a catched id.
          bool _connected_with_cached_id;

          Preferences* _preferences;

          friend class Manager;
      };

      // Until our wifi manager is connected, this state represents doing nothing.
      struct Disconnected final {
        uint8_t tick = 0;

        bool update(std::optional<wifimanager::Manager::EManagerMessage> &message);
      };

      // Redis configuration values.
      const char * _redis_host;
      const uint32_t _redis_port;

      // A tuple containing the ACL information that will be used for our connection.
      std::pair<const char *, const char *> _redis_auth;

      // When our wifi experiences a disconnect, we will pause all behaviors.
      bool _paused;

      Preferences _preferences;

      // The underlying state machine of this redis manager.
      std::variant<Disconnected, Connected> _state;
  };

}

#endif

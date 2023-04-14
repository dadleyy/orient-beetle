#ifndef _REDIS_EVENTS_H
#define _REDIS_EVENTS_H 1

#include <variant>
#include <optional>
#include <WiFiClientSecure.h>
#include <Preferences.h>

#include "wifi-events.hpp"
#include "microtim.hpp"

namespace redisevents {
  
  class Events final {
    public:
      explicit Events(std::tuple<const char *, const uint32_t, std::pair<const char *, const char *>>);
      ~Events() = default;

      // Disable Copy
      Events(const Events &) = delete;
      Events & operator=(const Events &) = delete;

      // Disable Move
      Events(Events &&) = delete;
      Events& operator=(Events &&) = delete;

      enum EMessage {
        FailedConnection,
        ConnectionLost,
        EstablishedConnection,
        IdentificationReceived,
        ReceivedMessage,
      };


      // Initialization - prepare our internal state
      void begin(void);

      // Update - given a message from the wifi manager and the current time, perform logic
      // based on our current state.
      std::optional<EMessage> update(std::optional<wifievents::Events::EMessage>&, uint32_t);

      // Copy the latest message (if any) into the destination.
      uint16_t copy(char *, uint16_t);

      // Return the size of our id.
      uint8_t id_size(void);
      uint8_t copy_id(char *, uint8_t);

    private:
      constexpr static const uint32_t FRAMEBUFFER_SIZE = 1024;
      constexpr static const uint32_t PARSED_MESSAGE_SIZE = 1024;
      constexpr static const uint32_t MAX_ID_SIZE = 36;

      // The amount of attempts to read from our tls connection that return no data
      // before we attempt to reconnect.
      constexpr static const uint32_t MAX_EMPTY_READ_RESET = 100;

      constexpr static const uint8_t OUTBOUND_BUFFER_SIZE = 200;

      // The amount of times we reset our connection before we will re-request a new device id.
      constexpr static const uint8_t MAX_RESETS_RECREDENTIALIZE = 5;

      constexpr static const char * EMPTY_STRING_RESPONSE = "$-1\r\n";
      constexpr static const char * EMPTY_ARRAY_RESPONSE = "*-1\r\n";
      constexpr static const char * OK = "+OK\r\n";
      constexpr static const char * PUSH_OK = ":1\r\n";
      constexpr static const char * WRONG_PASS_ERR = "-WRONGPASS invalid username-password pair or user is disabled\r\n";
      constexpr static const char * NO_PERM_ERR = "-NOPERM this user has no permissions to run the 'rpush' command or its subcommand\r\n";

      // registration queues:
      // - `ob:r` -> device pulls its id down
      // - `ob:i` -> device notifies it is online
      constexpr static const char REDIS_REGISTRATION_POP [] = "*2\r\n$4\r\nLPOP\r\n$4\r\nob:r\r\n";

      enum EAuthorizationStage {
        NotRequested,             // <- connects + writes auth
        AuthorizationRequested,   // <- reads `+OK` (skipped if id is in preferences).
        AuthorizationReceived,    // <- writes registrar-pop
        IdentificationRequested,  // <- reads id (skipped if id is in preferences).
        AuthorizationAttempted,   // <- waiting for response from `AUTH`
        FullyAuthorized,          // <- reads messages
      };

      enum EResponseParserTransition {
        Noop,
        Failure,
        HasArray,
        Done,
        StartString,
        EndString,
      };

      enum EParseResult {
        ParsedFailure,
        ParsedMessage,
        ParsedOk,
        ParsedNothing,
      };

      struct ParserVisitor;

      // Initially, we will _either_ be parsing the length of an array to follow, or the length
      // of a bulk string.
      struct InitialParser final {
        InitialParser(): _kind(0), _total(0), _delim(0) {}
        ~InitialParser() = default;

        InitialParser(const InitialParser&) = delete;
        InitialParser& operator=(const InitialParser&) = delete;

        InitialParser(const InitialParser&& other):
          _kind(other._kind), _total(other._total), _delim(other._delim) {}
        const InitialParser& operator=(const InitialParser&& other) {
          this->_kind = other._kind;
          this->_total = other._total;
          this->_delim = other._delim;
          return std::move(*this);
        };

        mutable uint8_t _kind;
        mutable uint8_t _total;
        mutable uint8_t _delim;

        friend class ParserVisitor;
      };

      struct BulkStringParser final {
        explicit BulkStringParser(uint8_t size): _size(size), _seen(0), _terminating(false) {}
        ~BulkStringParser() = default;

        BulkStringParser(const BulkStringParser&) = delete;
        BulkStringParser& operator=(const BulkStringParser&) = delete;

        BulkStringParser(const BulkStringParser&& other):
          _size(other._size), _seen(other._seen), _terminating(other._terminating) {}
        const BulkStringParser& operator=(const BulkStringParser&& other) {
          this->_terminating = other._terminating;
          this->_size = other._size;
          this->_seen = other._seen;
          return std::move(*this);
        };

        uint8_t _size;
        mutable uint8_t _seen;
        mutable bool _terminating;

        friend class ParserVisitor;
      };

      using ParserStates = std::variant<InitialParser, BulkStringParser>;

      struct ResponseParser final {
        ResponseParser(): _state(InitialParser()) {}
        EResponseParserTransition consume(char);
        ParserStates _state;
      };

      struct ParserVisitor final {
        ParserVisitor(char token): _token(token) {}

        std::tuple<ParserStates, EResponseParserTransition> operator()(const BulkStringParser&& initial) const;
        std::tuple<ParserStates, EResponseParserTransition> operator()(const InitialParser&& initial) const;

        char _token;
      };


      // Internal State Variant: Connected
      //
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
          std::optional<EMessage> update(
            const char *,
            const std::pair<const char *, const char *>&,
            uint32_t,
            uint32_t
          );

          void reset(void);

        private:
          std::optional<EMessage> connect(
            const char *,
            const std::pair<const char *, const char *>&,
            uint32_t
          );
          inline uint16_t write_message(uint32_t);
          inline EParseResult parse_framebuffer(void);

          EAuthorizationStage _authorization_stage;

          // The `_cursor` represents the last index of our framebuffer we have pushed into.
          // It is _also_ truncated down to the value of successfully parsed messages.
          uint16_t _cursor;

          ResponseParser _parser;

          // This memory is filled every update with the contents of our tcp connection.
          char * _framebuffer;
          char * _outbound_buffer;
          char * _parse_buffer;

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

          uint8_t _strange_thing_count = 0;

          microtim::MicroTimer _timer = microtim::MicroTimer(100);
          microtim::MicroTimer _write_timer = microtim::MicroTimer(500);
          bool _pending_response = false;
          bool _last_written_pop = false;

          friend class Events;
      };

      // Internal State Variant: Disconnected
      //
      // Until our wifi manager is connected, this state represents doing nothing.
      struct Disconnected final {
        bool update(std::optional<wifievents::Events::EMessage> &message);
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

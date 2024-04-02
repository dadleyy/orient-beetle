#pragma once

#include <WiFiClientSecure.h>

#include "redis-config.hpp"
#include "redis-event.hpp"
#include "redis-reader.hpp"
#include "wifi-events.hpp"

namespace redisevents {
extern const uint8_t redis_root_ca[] asm(
    "_binary_embeds_redis_host_root_ca_pem_start");

constexpr static const uint32_t MAX_ID_SIZE = 36;
constexpr static const uint8_t OUTBOUND_BUFFER_SIZE = 200;

template <std::size_t T>
class Events final {
 public:
  explicit Events(std::shared_ptr<RedisConfig> config)
      : _context(std::make_shared<Context>(config)),
        _state(Disconnected{}),
        _reader(std::make_shared<RedisReader<T>>()) {}

  Events(const Events &) = delete;
  Events &operator=(const Events &) = delete;
  Events(Events &&) = delete;
  Events &operator=(Events &&) = delete;

  void begin(void) { _context->preferences.begin("beetle-redis", false); }

  std::optional<RedisEvent> update(
      std::optional<wifievents::Events::EMessage> &wifi,
      std::shared_ptr<std::array<uint8_t, T>> buffer, uint32_t time) {
    auto visitor = StateVisitor{_context, &wifi, buffer, time, _reader};
    auto [next, message] = std::visit(visitor, _state);
    _state = next;
    return message;
  }

  // Return the size of our id.
  uint8_t id_size(void) { return _context->device_id_len; }

 private:
  struct Context;
  struct Disconnected;
  struct Connected;

  constexpr static const char REDIS_REGISTRATION_POP[] =
      "*2\r\n$4\r\nLPOP\r\n$4\r\nob:r\r\n";

  constexpr static const char REDIS_AUTH_FAILURE[] =
      "WRONGPASS invalid username-password pair or user is disabled.";

  enum AuthorizationStage {
    NotRequested,             // <- connects + writes auth
    AuthorizationRequested,   // <- reads `+OK` (skipped if id is in
                              // preferences).
    AuthorizationReceived,    // <- writes registrar-pop
    IdentificationRequested,  // <- reads id (skipped if id is in preferences).
    AuthorizationAttempted,   // <- waiting for response from `AUTH`
    FullyAuthorized,          // <- reads messages
  };

  struct Context final {
    explicit Context(std::shared_ptr<RedisConfig> config)
        : config(config),
          device_id((char *)malloc(sizeof(char) * MAX_ID_SIZE)),
          outbound((char *)malloc(sizeof(char) * OUTBOUND_BUFFER_SIZE)),
          device_id_len(0) {
      memset(device_id, '\0', MAX_ID_SIZE);
    }

    ~Context() {
      if (outbound != nullptr) {
        free(outbound);
      }
      outbound = nullptr;
      if (device_id != nullptr) {
        free(device_id);
      }
      device_id = nullptr;
    }

    // No copies; the context is passed around using a `std::shared_ptr`
    Context(Context &) = delete;
    Context(const Context &) = delete;
    Context &operator=(Context &) = delete;
    Context &operator=(const Context &) = delete;

    // The client instance that is responsible for writing and reading out
    // messages
    WiFiClientSecure client;

    // The redis config, including port, host, and burn-in credentials>
    std::shared_ptr<RedisConfig> config;

    // The handle to our persistent memory.
    Preferences preferences;

    // Some memory allocated for holding a device id.
    char *device_id;

    // Some memory allocated for holding our outbound messages.
    char *outbound;

    // The length of our id.
    uint8_t device_id_len;
  };

  /**
   * This struct is used to `std::visit` the connected/disconnected state held
   * by our main `RedisEvents` instance.
   */
  struct StateVisitor final {
    // The context holds our wifi client and buffers.
    std::shared_ptr<Context> context;

    // The optional message from our `wifievents` library that any given update
    // is dealing with.
    std::optional<wifievents::Events::EMessage> *wifi_message;

    // The shared pointer to our "global" data buffer.
    std::shared_ptr<std::array<uint8_t, T>> buffer;

    // The current time from `milis()`
    uint32_t time;

    // A shared pointer to a `RedisReader` state machine.
    std::shared_ptr<RedisReader<T>> reader;

    /**
     * This message will attempt to read from our connected `WiFiClientSecure`
     * instance, expecting to find a device id that will immediately be used in
     * a fresh `AUTH` request.
     */
    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    read_id(Connected connected) {
      while (context->client.available()) {
        auto token = (char)context->client.read();
        auto event = reader->fill(token, buffer);

        if (std::holds_alternative<RedisRead>(event)) {
          auto read_event = std::get<RedisRead>(event);

          context->device_id_len = read_event.size;
          memcpy(context->device_id, buffer->data(), read_event.size);

          context->preferences.putString("device-id", context->device_id);

          log_i("read %d bytes during id request: '%s'", context->device_id_len,
                context->device_id);

          memset(context->outbound, '\0', OUTBOUND_BUFFER_SIZE);

          sprintf(context->outbound,
                  "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
                  context->device_id_len, context->device_id,
                  context->device_id_len, context->device_id);
          context->client.print(context->outbound);
          log_i("wrote auth: '%s'", context->outbound);
          connected.authorization_stage =
              AuthorizationStage::AuthorizationAttempted;

          return std::make_pair(connected, std::nullopt);
        }
      }

      return std::make_pair(connected, std::nullopt);
    }

    /**
     * Writes the `POP` message that will give us a new device-specific id that
     * we will subsequently authorize with.
     */
    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    request_id(Connected connected) {
      log_i("authorized as burn-in, writing pop for new id");
      buffer->fill('\0');

      context->client.print(REDIS_REGISTRATION_POP);
      connected.authorization_stage =
          AuthorizationStage::IdentificationRequested;

      return std::make_pair(connected, std::nullopt);
    }

    /**
     * Expects to read `OK` from the wifi client; we will use this while waiting
     * for:
     * 1. auth burn-in `AUTH` request
     * 2. the device-specific `AUTH` request
     */
    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    read_ok(Connected connected) {
      bool pending_burnin_auth = connected.authorization_stage ==
                                 AuthorizationStage::AuthorizationRequested;

      if (connected.last_read == 0) {
        connected.last_read = time;
      }

      while (context->client.available()) {
        auto token = (char)context->client.read();
        auto event = reader->fill(token, buffer);

        if (std::holds_alternative<RedisRead>(event)) {
          auto read_len = std::get<RedisRead>(event);

          if (strcmp((char *)buffer->data(), "OK") == 0) {
            log_i("auth success of %d bytes, moving into pulling",
                  read_len.size);

            connected.authorization_stage =
                pending_burnin_auth ? AuthorizationStage::AuthorizationReceived
                                    : AuthorizationStage::FullyAuthorized;
          } else if (strcmp((char *)buffer->data(), REDIS_AUTH_FAILURE) == 0) {
            log_e("failed authenticating using current credentials");
            context->preferences.remove("device-id");
          } else {
            log_e("unrecognized response from redis - %s", buffer->data());
          }
        }

        connected.last_read = time;
      }

      if (connected.authorization_stage ==
          AuthorizationStage::FullyAuthorized) {
        return std::make_pair(connected, Authorized{});
      }

      if (time - connected.last_read > 5000) {
        log_e("expected OK from redis but none was received in time, aborting");
        // important: explicitly stopping the client frees up internal memory
        // used on the next connection attempt.
        context->client.stop();
        return std::make_pair(Connected{false}, FailedConnection{});
      }

      return std::make_pair(connected, std::nullopt);
    }

    /**
     * When we are first `Connected`, we need to start our wifi client, and
     * immediately perform and authorization attempt using either:
     * 1. the burn in credentials, if no device id is stored on the chip
     * 2. the device id, if one is found on the chip
     */
    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    initial_auth(Connected connected) {
      context->client.setCACert((char *)redis_root_ca);
      int result =
          context->client.connect(context->config->host, context->config->port);

      if (result != 1) {
        log_e("unable to establish connection - %d", result);
        context->client.stop();
        return std::make_pair(Disconnected{time + 5000}, FailedConnection{});
      }

      log_i("redis connection established successfully");

      size_t stored_id_len =
          context->preferences.isKey("device-id")
              ? context->preferences.getString("device-id", context->device_id,
                                               MAX_ID_SIZE)
              : 0;

      buffer->fill('\0');
      memset(context->outbound, '\0', OUTBOUND_BUFFER_SIZE);

      // If we have a stored id, try using it for an `AUTH`
      if (stored_id_len > 0) {
        log_i("device id loaded from non-volatile memory: '%s'",
              context->device_id);
        context->device_id_len = stored_id_len - 1;
        sprintf(context->outbound,
                "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
                context->device_id_len, context->device_id,
                context->device_id_len, context->device_id);
        context->client.print(context->outbound);
        log_d("wrote auth: '%s'; clearing internal buffer", context->outbound);
        connected.authorization_stage =
            AuthorizationStage::AuthorizationAttempted;
        return std::make_pair(connected, IdentificationReceived{});
      }

      // If we do not have a stored id, we're going to try authorizing with
      // the burn in credentials which will allow us to request a fresh
      // device id.
      auto burnin_name = std::get<0>(context->config->auth);
      auto burnin_password = std::get<1>(context->config->auth);

      sprintf(context->outbound,
              "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
              strlen(burnin_name), burnin_name, strlen(burnin_password),
              burnin_password);

      log_i("no stored device id, attempting to request one: %s",
            context->outbound);

      context->client.print(context->outbound);
      connected.authorization_stage =
          AuthorizationStage::AuthorizationRequested;
      return std::make_pair(connected, IdentificationReceived{});
    }

    /**
     * The main working loop - here we are moving between reading and writing,
     * where our writes will either be a request for the next message from our
     * queue, or a "heartbeat" push into the incoming queue.
     *
     * After each write, we will read for the next message.
     */
    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    work(Connected connected) {
      // If we're waiting for a response, read the message.
      if (std::holds_alternative<ReceivingPop>(connected.state) ||
          std::holds_alternative<ReceivingHeartbeatAck>(connected.state)) {
        uint32_t bytes_read = 0;

        while (context->client.available()) {
          auto token = (char)context->client.read();
          auto event = reader->fill(token, buffer);
          bytes_read += 1;

          if (std::holds_alternative<RedisInt>(event) &&
              std::holds_alternative<ReceivingHeartbeatAck>(connected.state)) {
            log_i("heartbeat ACK received");
            connected.state = NotReceiving{false};
          }

          if (std::holds_alternative<RedisArray>(event) &&
              std::holds_alternative<ReceivingPop>(connected.state)) {
            auto array_read = std::get<RedisArray>(event);

            // If we ready an array response but the length is -1 or 0,
            // we're no longer expecting any messages
            if (array_read.size == -1 || array_read.size == 0) {
              log_i("empty array received while waiting for message pop");
              connected.state = NotReceiving{true};
              return std::make_pair(connected, std::nullopt);
            }

            log_i("expecting %d messages to follow initial array read",
                  array_read.size);

            connected.state = ReceivingPop{array_read.size, 0};
          }

          if (std::holds_alternative<RedisRead>(event) &&
              std::holds_alternative<ReceivingPop>(connected.state)) {
            auto payload_count =
                std::get_if<ReceivingPop>(&connected.state)->payload_count;
            bool had_payload = payload_count > 0;
            auto position =
                std::get_if<ReceivingPop>(&connected.state)->payload_position;

            auto read_result = std::get<RedisRead>(event);

            connected.state = ReceivingPop{payload_count, position + 1};

            log_i("received read event of size %d on payload item %d",
                  read_result.size, payload_count);

            if (had_payload && position + 1 == payload_count) {
              connected.state = NotReceiving{true};
              log_i("finished all array elements, last size: %d (of %d)",
                    read_result.size, T);
              RedisEvent event = PayloadReceived{(uint32_t)read_result.size};
              return std::make_pair(connected, event);
            }
          }
        }

        if (std::holds_alternative<ReceivingPop>(connected.state)) {
          ReceivingPop *receiver = std::get_if<ReceivingPop>(&connected.state);
          uint32_t time_diff = time - receiver->timeout_start;

          if (time_diff > 1000) {
            log_i("still waiting for redis reponse data after %d reads",
                  receiver->pending_reads);
            receiver->timeout_start = time;
            receiver->pending_reads += 1;
          }
        }

        return std::make_pair(connected, std::nullopt);
      }

      // Do nothing when we're not ready to write the next message
      if (time - connected.last_write < 2000) {
        return std::make_pair(connected, std::nullopt);
      }

      // Start our write by clearing out our current buffer.
      buffer->fill('\0');

      if (std::holds_alternative<NotReceiving>(connected.state)) {
        auto sending_heartbeat =
            std::get_if<NotReceiving>(&connected.state)->heartbeat_next;

        connected.last_write = time;
        memset(context->outbound, '\0', OUTBOUND_BUFFER_SIZE);

        if (sending_heartbeat) {
          connected.state = ReceivingHeartbeatAck{};
          sprintf(context->outbound,
                  "*3\r\n$5\r\nRPUSH\r\n$4\r\nob:i\r\n$%d\r\n%s\r\n",
                  context->device_id_len, context->device_id);
        } else {
          connected.state = ReceivingPop{};
          sprintf(context->outbound,
                  "*3\r\n$5\r\nBLPOP\r\n$%d\r\nob:%s\r\n$1\r\n5\r\n",
                  context->device_id_len + 3, context->device_id);
        }

        log_i("id[%s] writing message (heartbeat? %d)", context->device_id,
              sending_heartbeat);
        context->client.print(context->outbound);
      }

      return std::make_pair(connected, std::nullopt);
    }

    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    operator()(Connected connected) {
      if (*wifi_message == wifievents::Events::EMessage::Disconnected) {
        context->client.stop();
        return std::make_pair(Disconnected{}, std::nullopt);
      }

      if (*wifi_message ==
          wifievents::Events::EMessage::ConnectionInterruption) {
        connected.paused = true;
        return std::make_pair(connected, std::nullopt);
      }

      if (connected.paused) {
        if (*wifi_message == wifievents::Events::EMessage::ConnectionResumed) {
          context->client.stop();
          return std::make_pair(Disconnected{}, std::nullopt);
        }

        return std::make_pair(connected, std::nullopt);
      }

      switch (connected.authorization_stage) {
        case AuthorizationStage::IdentificationRequested:
          return read_id(connected);

        case AuthorizationStage::AuthorizationReceived:
          return request_id(connected);

        case AuthorizationStage::FullyAuthorized:
          return work(connected);

        case AuthorizationStage::AuthorizationRequested:
        case AuthorizationStage::AuthorizationAttempted:
          // TODO: it is likely that we should be ensuring the buffer is cleared
          // out _before_ moving into either of these authorization states.
          buffer->fill('\0');
          return read_ok(connected);

        case AuthorizationStage::NotRequested:
          return initial_auth(connected);
      }

      return std::make_pair(connected, std::nullopt);
    }

    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    operator()(Disconnected d) {
      bool reconnect =
          *wifi_message == wifievents::Events::EMessage::Connected ||
          *wifi_message == wifievents::Events::EMessage::ConnectionResumed;

      if (d.reconnect_after > 0 && time > d.reconnect_after) {
        log_i("explicit redis reconnection attempt");
        return std::make_pair(Connected{false}, std::nullopt);
      }

      if (reconnect) {
        log_i("redis events moving into connection attempt");
        return std::make_pair(Connected{false}, std::nullopt);
      }

      if (d.last_debug == 0) {
        d.last_debug = time;
      }

      if (time - d.last_debug > 3000) {
        log_e("redis events disconnected; no connected wifi events received");
        d.last_debug = time;
      }

      return std::make_pair(d, std::nullopt);
    }
  };

  struct Disconnected final {
    uint32_t reconnect_after = 0;
    uint32_t last_debug = 0;
  };

  struct ReceivingHeartbeatAck final {};

  /**
   * After attempting to pop a message from our queue, this struct is used to
   * maintain state since messages may be large enough that they arrive in
   * multiple read attempts.
   */
  struct ReceivingPop final {
    int32_t payload_count = 0;
    int32_t payload_position = 0;
    uint32_t timeout_start = 0;
    uint32_t pending_reads = 0;
  };

  struct NotReceiving final {
    bool heartbeat_next;
  };

  struct Connected final {
    bool paused = false;
    uint32_t last_write = 0;
    uint32_t last_read = 0;
    AuthorizationStage authorization_stage = AuthorizationStage::NotRequested;
    std::variant<ReceivingHeartbeatAck, ReceivingPop, NotReceiving> state =
        NotReceiving{true};
  };

  std::shared_ptr<Context> _context;
  std::variant<Disconnected, Connected> _state;
  std::shared_ptr<RedisReader<T>> _reader;
};
}  // namespace redisevents

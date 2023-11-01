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
    auto[next, message] = std::visit(visitor, _state);
    _state = next;
    return message;
  }

  // Return the size of our id.
  uint8_t id_size(void) { return _context->device_id_len; }

  uint8_t copy_id(char *out, uint8_t max) { return max; }

 private:
  struct Context;
  struct Disconnected;
  struct Connected;

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

    Context(Context &) = delete;
    Context(const Context &) = delete;
    Context &operator=(Context &) = delete;
    Context &operator=(const Context &) = delete;

    WiFiClientSecure client;
    std::shared_ptr<RedisConfig> config;
    Preferences preferences;

    char *device_id;
    char *outbound;

    uint8_t device_id_len;
  };

  struct StateVisitor final {
    std::shared_ptr<Context> context;
    std::optional<wifievents::Events::EMessage> *wifi_message;
    std::shared_ptr<std::array<uint8_t, T>> buffer;
    uint32_t time;
    std::shared_ptr<RedisReader<T>> reader;

    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    operator()(Connected c) {
      if (*wifi_message == wifievents::Events::EMessage::Disconnected) {
        return std::make_pair(Disconnected{}, std::nullopt);
      }

      if (*wifi_message ==
          wifievents::Events::EMessage::ConnectionInterruption) {
        c.paused = true;
        return std::make_pair(c, std::nullopt);
      }

      if (c.paused) {
        if (*wifi_message == wifievents::Events::EMessage::ConnectionResumed) {
          return std::make_pair(Disconnected{}, std::nullopt);
        }

        return std::make_pair(c, std::nullopt);
      }

      switch (c.authorization_stage) {
        case AuthorizationStage::IdentificationRequested:
        case AuthorizationStage::AuthorizationRequested:
        case AuthorizationStage::AuthorizationReceived:
          return std::make_pair(c, std::nullopt);

        case AuthorizationStage::FullyAuthorized: {
          // If we're waiting for a response, read the message.
          if (std::holds_alternative<ReceivingPop>(c.state) ||
              std::holds_alternative<ReceivingHeartbeatAck>(c.state)) {
            uint32_t bytes_read = 0;

            while (context->client.available()) {
              auto token = (char)context->client.read();
              auto event = reader->fill(token, buffer);
              bytes_read += 1;

              if (std::holds_alternative<RedisInt>(event) &&
                  std::holds_alternative<ReceivingHeartbeatAck>(c.state)) {
                log_i("heartbeat ACK received");
                c.state = NotReceiving{false};
              }

              if (std::holds_alternative<RedisArray>(event) &&
                  std::holds_alternative<ReceivingPop>(c.state)) {
                auto array_read = std::get<RedisArray>(event);

                // If we ready an array response but the length is -1 or 0,
                // we're no longer expecting any messages
                if (array_read.size == -1 || array_read.size == 0) {
                  log_i("empty array received while waiting for message pop");
                  c.state = NotReceiving{true};
                  return std::make_pair(c, std::nullopt);
                }

                log_i("expecting %d messages to follow initial array read",
                      array_read.size);

                c.state = ReceivingPop{array_read.size, 0};
              }

              if (std::holds_alternative<RedisRead>(event) &&
                  std::holds_alternative<ReceivingPop>(c.state)) {
                auto payload_count =
                    std::get_if<ReceivingPop>(&c.state)->payload_count;
                bool had_payload = payload_count > 0;
                auto position =
                    std::get_if<ReceivingPop>(&c.state)->payload_position;

                auto read_result = std::get<RedisRead>(event);

                c.state = ReceivingPop{payload_count, position + 1};

                log_i("received read event of size %d on payload item %d",
                      read_result.size, payload_count);

                if (had_payload && position + 1 == payload_count) {
                  c.state = NotReceiving{true};
                  log_i("finished all array elements, last size: %d (of %d)",
                        read_result.size, T);
                  RedisEvent e = PayloadReceived{(uint32_t)read_result.size};
                  return std::make_pair(c, e);
                }
              }
            }

            if (std::holds_alternative<ReceivingPop>(c.state)) {
              ReceivingPop *receiver = std::get_if<ReceivingPop>(&c.state);
              uint32_t time_diff = time - receiver->timeout_start;

              if (time_diff > 1000) {
                log_i("still waiting for redis reponse data after %d reads",
                      receiver->pending_reads);
                receiver->timeout_start = time;
                receiver->pending_reads += 1;
              }
            }

            return std::make_pair(c, std::nullopt);
          }

          if (time - c.last_write < 2000) {
            return std::make_pair(c, std::nullopt);
          }

          buffer->fill('\0');

          if (std::holds_alternative<NotReceiving>(c.state)) {
            auto sending_heartbeat =
                std::get_if<NotReceiving>(&c.state)->heartbeat_next;

            c.last_write = time;
            memset(context->outbound, '\0', OUTBOUND_BUFFER_SIZE);

            if (sending_heartbeat) {
              c.state = ReceivingHeartbeatAck{};
              sprintf(context->outbound,
                      "*3\r\n$5\r\nRPUSH\r\n$4\r\nob:i\r\n$%d\r\n%s\r\n",
                      context->device_id_len, context->device_id);
            } else {
              c.state = ReceivingPop{};
              sprintf(context->outbound,
                      "*3\r\n$5\r\nBLPOP\r\n$%d\r\nob:%s\r\n$1\r\n5\r\n",
                      context->device_id_len + 3, context->device_id);
            }

            log_i("writing message (heartbeat? %d)", sending_heartbeat);
            context->client.print(context->outbound);
          }

          return std::make_pair(c, std::nullopt);
        }

        case AuthorizationStage::AuthorizationAttempted: {
          buffer->fill('\0');

          while (context->client.available()) {
            auto token = (char)context->client.read();
            auto event = reader->fill(token, buffer);

            if (std::holds_alternative<RedisRead>(event)) {
              auto read_len = std::get<RedisRead>(event);

              if (strcmp((char *)buffer->data(), "OK") == 0) {
                log_i("auth success of %d bytes, moving into pulling",
                      read_len.size);
                c.authorization_stage = AuthorizationStage::FullyAuthorized;
              }
            }
          }

          return std::make_pair(c, std::nullopt);
        }

        case AuthorizationStage::NotRequested: {
          log_i("redis not yet requested any auth");
          c.authorization_stage = AuthorizationStage::AuthorizationRequested;
          context->client.setCACert((char *)redis_root_ca);
          int result = context->client.connect(context->config->host,
                                               context->config->port);

          if (result != 1) {
            log_e("unable to establish connection - %d", result);
            return std::make_pair(Disconnected{}, FailedConnection{});
          }

          log_i("redis connection established successfully");

          size_t stored_id_len =
              context->preferences.isKey("device-id")
                  ? context->preferences.getString(
                        "device-id", context->device_id, MAX_ID_SIZE)
                  : 0;

          if (stored_id_len > 0) {
            log_i("device id loaded from non-volatile memory: '%s'",
                  context->device_id);
            context->device_id_len = stored_id_len - 1;
            memset(context->outbound, '\0', OUTBOUND_BUFFER_SIZE);
            sprintf(context->outbound,
                    "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
                    context->device_id_len, context->device_id,
                    context->device_id_len, context->device_id);
            context->client.print(context->outbound);
            log_d("wrote auth: '%s'", context->outbound);
            c.authorization_stage = AuthorizationStage::AuthorizationAttempted;
            return std::make_pair(c, IdentificationReceived{});
          }

          log_i("no stored device id, attempting to request one");
        }
      }

      return std::make_pair(c, std::nullopt);
    }

    std::pair<std::variant<Disconnected, Connected>, std::optional<RedisEvent>>
    operator()(Disconnected d) {
      if (*wifi_message == wifievents::Events::EMessage::Connected) {
        log_i("redis events moving into connection attempt");
        return std::make_pair(Connected{false}, std::nullopt);
      }

      return std::make_pair(Disconnected{}, std::nullopt);
    }
  };

  struct Disconnected final {};

  struct ReceivingHeartbeatAck final {};

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
    AuthorizationStage authorization_stage = AuthorizationStage::NotRequested;
    std::variant<ReceivingHeartbeatAck, ReceivingPop, NotReceiving> state =
        NotReceiving{true};
  };

  std::shared_ptr<Context> _context;
  std::variant<Disconnected, Connected> _state;
  std::shared_ptr<RedisReader<T>> _reader;
};
}

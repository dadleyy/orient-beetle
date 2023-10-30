#ifndef _ENGINE_H
#define _ENGINE_H 1

#include <optional>
#include "redis-config.hpp"
#include "redis-events.hpp"
#include "state.hpp"
#include "wifi-events.hpp"

class Engine final {
 public:
  Engine(std::tuple<const char *, const char *>,
         std::shared_ptr<redisevents::RedisConfig>);
  ~Engine() = default;

  Engine() = delete;
  Engine(const Engine &) = delete;
  Engine &operator=(const Engine &) = delete;

  void begin(void);
  states::State update(states::State &&, uint32_t);

 private:
  std::shared_ptr<std::array<uint8_t, states::BUFFER_SIZE>> _buffer;
  wifievents::Events _wifi;
  redisevents::Events<states::BUFFER_SIZE> _redis;
};

#endif

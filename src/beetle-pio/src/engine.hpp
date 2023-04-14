#ifndef _ENGINE_H
#define _ENGINE_H 1

#include <optional>
#include "wifi-events.hpp"
#include "redis-events.hpp"
#include "state.hpp"

class Engine final {
  public:
    Engine(
      std::tuple<const char *, const char *>,
      std::tuple<const char *, uint32_t, std::pair<const char *, const char *>>
    );
    ~Engine() = default;

    Engine() = delete;
    Engine(const Engine&) = delete;
    Engine& operator=(const Engine&) = delete;

    void begin(void);
    State update(State&, uint32_t);

  private:
    wifievents::Events _wifi;
    redisevents::Events _redis;
};

#endif

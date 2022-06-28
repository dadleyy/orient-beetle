#ifndef _ENGINE_H
#define _ENGINE_H 1

#include <optional>
#include "wifi-manager.hpp"
#include "redis-manager.hpp"
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
    State update(State&);

  private:
    wifimanager::Manager _wifi;
    redismanager::Manager _redis;
};

#endif

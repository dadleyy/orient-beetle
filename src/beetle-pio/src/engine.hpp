#ifndef _ENGINE_H
#define _ENGINE_H 1

#include <optional>
#include "wifi-manager.hpp"
#include "redis-manager.hpp"

class Engine final {
  public:
    Engine(
      std::tuple<const char *, const char *>,
      std::tuple<const char *, uint32_t, const char *>
    );
    ~Engine() = default;

    Engine() = delete;
    Engine(const Engine&) = delete;
    Engine& operator=(const Engine&) = delete;

    void begin(void);
    void update(void);
    void view(char *, uint16_t);

  private:
    constexpr static const uint16_t view_buffer_size = 1024;

    enum EEngineMode {
      Idle,
      ConnectingWifi,
      Working,
    };

    char _buffer[view_buffer_size];
    uint16_t _buffer_len = 0;

    EEngineMode _mode = EEngineMode::Idle;
    uint8_t _tick = 0;

    wifimanager::Manager _wifi;
    redismanager::Manager _redis;
};

#endif

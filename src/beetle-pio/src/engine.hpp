#ifndef _ENGINE_H
#define _ENGINE_H 1

#include <optional>
#include "wifi-manager.hpp"
#include "redis-manager.hpp"

class Engine final {
  public:
    Engine() = default;
    ~Engine() = default;

    void update(wifimanager::Manager&, redismanager::Manager&);
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
};

#endif

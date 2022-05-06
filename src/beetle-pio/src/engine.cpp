#include "engine.hpp"

Engine::Engine(
  std::tuple<const char *, const char *> ap_config,
  std::tuple<const char *, uint32_t, const char *> redis_config
): _wifi(ap_config), _redis(redis_config) {
}

void Engine::begin(void) {
  _wifi.begin();
}

State Engine::update(State& current) {
  State next(std::move(current));

  std::optional<wifimanager::Manager::EManagerMessage> wifi_update = _wifi.frame();
  std::optional<redismanager::Manager::EManagerMessage> redis_update = _redis.frame(wifi_update);

  if (wifi_update) {
    switch (*wifi_update) {
      case wifimanager::Manager::EManagerMessage::Connecting:
        next.active.emplace<ConnectingState>();
        return next;
      case wifimanager::Manager::EManagerMessage::FailedConnection:
        return next;
      case wifimanager::Manager::EManagerMessage::Disconnected:
        next.active.emplace<ConfiguringState>();
        return next;
      case wifimanager::Manager::EManagerMessage::ConnectionInterruption:
        return next;
      case wifimanager::Manager::EManagerMessage::ConnectionResumed:
        return next;
      case wifimanager::Manager::EManagerMessage::Connected:
        next.active.emplace<ConnectedState>();
        return next;
    }
  }

  // If we havent received a new message and are connecting, just jump the attempt
  // count and move along.
  if (std::get_if<ConnectingState>(&next.active) != nullptr) {
    next.active.emplace<ConnectingState>(_wifi.attempt());
    return next;
  }

  return next;
}

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

  if (wifi_update) {
    switch (*wifi_update) {
      case wifimanager::Manager::EManagerMessage::Connecting:
        next.active.emplace<ConnectingState>();
        return next;
      case wifimanager::Manager::EManagerMessage::FailedConnection:
        break;
      case wifimanager::Manager::EManagerMessage::Disconnected:
        break;
      case wifimanager::Manager::EManagerMessage::ConnectionInterruption:
        break;
      case wifimanager::Manager::EManagerMessage::ConnectionResumed:
        break;
      case wifimanager::Manager::EManagerMessage::Connected:
        next.active.emplace<ConnectedState>();
        return next;
    }
  }

  std::optional<redismanager::Manager::EManagerMessage> redis_update = _redis.frame(wifi_update);

  return next;
}

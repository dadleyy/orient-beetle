#include "engine.hpp"

Engine::Engine(
  std::tuple<const char *, const char *> ap_config,
  std::tuple<const char *, uint32_t, std::pair<const char *, const char *>> redis_config
): _wifi(ap_config), _redis(redis_config) {
}

void Engine::begin(void) {
  _wifi.begin();
  _redis.begin();
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

      case wifimanager::Manager::EManagerMessage::Disconnected:
        next.active.emplace<ConfiguringState>();
        return next;

      case wifimanager::Manager::EManagerMessage::ConnectionResumed:
      case wifimanager::Manager::EManagerMessage::Connected:
        next.active.emplace<ConnectedState>();
        return next;

      case wifimanager::Manager::EManagerMessage::ConnectionInterruption:
      case wifimanager::Manager::EManagerMessage::FailedConnection:
        next.active.emplace<UnknownState>();
        return next;
    }
  }

  // While we are in a connecting state, make sure to _not_ "leave" until we have received
  // `Connected` from the wifi manager, indicating we're back online.
  if (std::get_if<ConnectingState>(&next.active) != nullptr) {
    next.active.emplace<ConnectingState>(_wifi.attempt());
    return next;
  }

  // If redis has received an id and we had previously moved into a `Connected` state, we
  // should now enter our main, `Working` state that will hold messages.
  bool now_working = redis_update ==
    redismanager::Manager::EManagerMessage::IdentificationReceived
    && std::get_if<ConnectedState>(&next.active) != nullptr;

  if (now_working) {
    next.active.emplace<WorkingState>(_redis.id_size());
    WorkingState * w = std::get_if<WorkingState>(&next.active);
    log_d("moved into working state with id size '%d' -> '%s'", _redis.id_size(), w->id_content);
    _redis.copy_id(w->id_content, _redis.id_size());
    return next;
  }

  bool has_message =
    redis_update == redismanager::Manager::EManagerMessage::ReceivedMessage
    && std::get_if<WorkingState>(&next.active);

  if (has_message) {
    WorkingState * w = std::get_if<WorkingState>(&next.active);

    if (!w) {
      log_e("received message, but redis not yet connected. strange");
      return next;
    }

    log_d("received message, copying buffer to connected state");

    Message& next = w->next();
    next.content_size = _redis.copy(next.content, 2048);
  }

  return next;
}

#include "engine.hpp"

Engine::Engine(
  std::tuple<const char *, const char *> ap_config,
  std::tuple<const char *, uint32_t, std::pair<const char *, const char *>> redis_config
): _wifi(ap_config), _redis(redis_config) {
}

// Prepares the wifi server and memory locations for our persistent data like SSID,
// password and redis credentials.
void Engine::begin(void) {
  _wifi.begin();
  _redis.begin();
}

// Given a reference to a state, this method will poll events on both the wifi an redis
// channels, attempting to update the state.
states::State Engine::update(states::State&& current, uint32_t current_time) {
  states::State next(std::move(current));

  std::optional<wifievents::Events::EMessage> wifi_update = _wifi.update(current_time);
  std::optional<redisevents::Events::EMessage> redis_update = _redis.update(wifi_update, current_time);

  if (wifi_update) {
    switch (*wifi_update) {
      case wifievents::Events::EMessage::Connecting:
        next.active.emplace<states::Connecting>();
        return next;

      case wifievents::Events::EMessage::Disconnected:
        next.active.emplace<states::Configuring>();
        return next;

      case wifievents::Events::EMessage::ConnectionResumed:
      case wifievents::Events::EMessage::Connected:
        next.active.emplace<states::Connected>();
        return next;

      case wifievents::Events::EMessage::ConnectionInterruption:
      case wifievents::Events::EMessage::FailedConnection:
        next.active.emplace<states::Unknown>();
        return next;
    }
  }

  // While we are in a connecting state, make sure to _not_ "leave" until we have received
  // `Connected` from the wifi manager, indicating we're back online.
  if (std::get_if<states::Connecting>(&next.active) != nullptr) {
    next.active.emplace<states::Connecting>(_wifi.attempt());
    return next;
  }

  // If redis has received an id and we had previously moved into a `Connected` state, we
  // should now enter our main, `Working` state that will hold messages.
  bool now_working = redis_update ==
    redisevents::Events::EMessage::IdentificationReceived
    && std::get_if<states::Connected>(&next.active) != nullptr;

  // Moving into the working state _will_ allocate memory; our message buffers.
  if (now_working) {
    next.active.emplace<states::Working>(_redis.id_size());
    states::Working * working_state = std::get_if<states::Working>(&next.active);
    _redis.copy_id(working_state->id_content, _redis.id_size());
    log_i("moved into working state with id size '%d' (id: '%s')", _redis.id_size(), working_state->id_content);
    return next;
  }

  bool has_message =
    redis_update == redisevents::Events::EMessage::ReceivedMessage
    && std::get_if<states::Working>(&next.active);

  // If we received a redis message update and we're currently "working", attempt to
  // copy our redis message into the next available string buffer.
  if (has_message) {
    states::Working * working_state = std::get_if<states::Working>(&next.active);

    if (!working_state) {
      log_e("received message, but redis not yet connected. strange");
      return next;
    }

    // Get a reference to the next message from our working state. We will use this to fill it
    // with the contents currently being held by the redis event buffer.
    log_i("received message, copying buffer to connected state");
    states::Message& next_message = working_state->next();
    next_message.size = _redis.copy(next_message.content, states::MAX_MESSAGE_SIZE);
    log_i("updated message size: '%d'", next_message.size);
  }

  return next;
}

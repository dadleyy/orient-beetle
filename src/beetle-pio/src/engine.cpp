#include "engine.hpp"

#ifndef BEETLE_VERSION
#define BEETLE_VERSION "dev"
#endif

Engine::Engine(std::tuple<const char*, const char*> ap_config,
               std::shared_ptr<redisevents::RedisConfig> redis_config)
    : _buffer(std::make_shared<std::array<uint8_t, states::BUFFER_SIZE>>()),
      _wifi(ap_config),
      _redis(redis_config) {}

// Prepares the wifi server and memory locations for our persistent data like
// SSID, password and redis credentials.
void Engine::begin(void) {
  _wifi.begin();
  _redis.begin();
}

// Given a reference to a state, this method will poll events on both the wifi
// and redis channels, attempting to update the state.
states::State Engine::update(states::State&& current, uint32_t current_time) {
  states::State next(std::move(current));

  auto wifi_update = _wifi.update(current_time);

  if (wifi_update != std::nullopt) {
    switch (*wifi_update) {
      case wifievents::Events::EMessage::AttemptingConnection:
        log_i("received connection attempt event from wifi");
        next = states::Connecting{};
        break;
      case wifievents::Events::EMessage::ConnectionResumed:
      case wifievents::Events::EMessage::Connected:
        log_i("received connection established event from wifi");
        next = states::Connected{};
        break;
      case wifievents::Events::EMessage::FailedConnection:
      case wifievents::Events::EMessage::ConnectionInterruption:
      case wifievents::Events::EMessage::Disconnected:
        log_i("received error event from wifi");
        next = states::Unknown{};
        break;
      case wifievents::Events::EMessage::WaitingForCredentials:
        log_i("acknowlegement of wifi waiting for credentials from user");
        next = states::Configuring{};
        break;
    }
  } else if (current_time - _last_time > 3000) {
    log_i("(v. %s) no update from wifi events", BEETLE_VERSION);
    _last_time = current_time;
  }

  auto redis_update = _redis.update(wifi_update, _buffer, current_time);

  if (redis_update != std::nullopt &&
      std::holds_alternative<redisevents::PayloadReceived>(*redis_update)) {
    auto payload_info = std::get<redisevents::PayloadReceived>(*redis_update);
    log_i("we have a payload of %d bytes from redis", payload_info.size);
    next = states::HoldingUpdate{_buffer, payload_info.size};
  } else if (redis_update != std::nullopt &&
             std::holds_alternative<redisevents::Authorized>(*redis_update)) {
    next = states::Working{};
  }

  return next;
}

#include "engine.hpp"

void Engine::update(wifimanager::Manager &wifi, redismanager::Manager &redis) {
  std::optional<wifimanager::Manager::EManagerMessage> wifi_update = wifi.frame();
  std::optional<redismanager::Manager::EManagerMessage> redis_update = redis.frame(wifi_update);

  if (wifi_update != std::nullopt) {
    switch (wifi_update.value()) {
      case wifimanager::Manager::EManagerMessage::Connecting:
        _mode = EEngineMode::ConnectingWifi;
        break;
      case wifimanager::Manager::EManagerMessage::FailedConnection:
        log_e("wifi manager failed connection");
        _mode = EEngineMode::Idle;
        break;
      case wifimanager::Manager::EManagerMessage::Disconnected:
        log_e("wifi manager disconnected");
        _mode = EEngineMode::Idle;
        break;

      case wifimanager::Manager::EManagerMessage::ConnectionInterruption:
      case wifimanager::Manager::EManagerMessage::ConnectionResumed:
      case wifimanager::Manager::EManagerMessage::Connected:
      default:
        break;
    }
  }

  if (redis_update != std::nullopt) {
    switch (redis_update.value()) {
      case redismanager::Manager::EManagerMessage::EstablishedConnection:
        log_d("redis manager was connected, moving into working");
        _mode = EEngineMode::Working;
        break;
      case redismanager::Manager::EManagerMessage::ReceivedMessage:
        log_d("appears to received message from redis");

        if (_mode == EEngineMode::Working) {
          log_d("copying message from redis manager in preparation for view");

          _buffer_len = redis.copy(_buffer, view_buffer_size);

          if (_buffer_len > 0) {
            log_d("received message from redis: %s", _buffer);
          }
        }
        break;

      // these messages are not necessarily interesting to the user, or are covered
      // by transitions ealier.
      case redismanager::Manager::EManagerMessage::ConnectionLost:
      case redismanager::Manager::EManagerMessage::FailedConnection:
      default:
        break;
    }
  }

  if (_mode != EEngineMode::Working) {
    _buffer_len = 0;
    memset(_buffer, '\0', view_buffer_size);
  }
}

void Engine::view(char * destination, uint16_t size) {
  switch (_mode) {
    case EEngineMode::Idle:
      strcpy(destination, "configuring");
      break;
    case EEngineMode::ConnectingWifi:
      strcpy(destination, "connecting");
      break;
    case EEngineMode::Working:
      if (_buffer_len > 0) {
        uint16_t amount = size < _buffer_len ? size : _buffer_len;
        memcpy(destination, _buffer, amount);
      } else {
        strcpy(destination, "working...");
      }
      break;
    default:
      strcpy(destination, "other");
      break;
  }
}

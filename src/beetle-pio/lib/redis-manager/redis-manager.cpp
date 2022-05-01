#include "redis-manager.hpp"

namespace redismanager {

  Manager::Manager(const char * host, const uint32_t port, const char * auth):
    _redis_host(host),
    _redis_port(port),
    _redis_auth(auth),
    _paused(false),
    _state(Disconnected { tick: 0 })
  {
  }

  uint16_t Manager::copy(char * destination, uint16_t max) {
    if (_state.index() != 1) {
      return 0;
    }

    Connected * c = std::get_if<1>(& _state);

    return c->copy(destination, max);
  }

  std::optional<Manager::EManagerMessage> Manager::frame(
      std::optional<wifimanager::Manager::EManagerMessage> &message
  ) {
    switch (_state.index()) {
      case 0: {
        Disconnected * d = std::get_if<0>(&_state);

        if (d->update(message) || _paused) {
#ifndef RELEASE
          log_d("attempting to move from disconnect to connected");
#endif

          _paused = false;
          _state.emplace<Connected>();
        }
        break;
      }
      case 1:
        Connected * c = std::get_if<1>(&_state);

        // If we've been completely disconnected, restart.
        if (message == wifimanager::Manager::EManagerMessage::Disconnected) {
          c->client.stop();
          _state.emplace<Disconnected>();
          _paused = false;
          return Manager::EManagerMessage::ConnectionLost;
        }

        // If we're paused and we've got anything but a resume, do nothing.
        if (_paused && message != wifimanager::Manager::EManagerMessage::ConnectionResumed) {
          return std::nullopt;
        }

        // If we've received an interruption, pause immediately.
        if (message == wifimanager::Manager::EManagerMessage::ConnectionInterruption) {
          _paused = true;
#ifndef RELEASE
          log_d("wifi connection was interrupted, pausing all requests");
#endif

          return std::nullopt;
        }

        // If we're paused and are about to resume, reset us to disconnected. The next frame will bounce us
        // back into connected.
        if (_paused && message == wifimanager::Manager::ConnectionResumed) {
          c->client.stop();
          _state.emplace<Disconnected>();
          return Manager::EManagerMessage::ConnectionLost;
        }

        return c->update(_redis_host, _redis_auth, _redis_port);
    }

    return std::nullopt;
  }

  uint16_t Manager::Connected::copy(char * destination, uint16_t size) {
    if (cursor == 0 || !certified) {
      return 0;
    }

    uint16_t amount = cursor < size ? cursor : size;
    memcpy(destination, framebuffer, amount);
    cursor = 0;
    memset(framebuffer, '\0', framebuffer_size);
    return amount;
  }

  std::optional<Manager::EManagerMessage> Manager::Connected::update(
      const char * _redis_host,
      const char * _redis_auth,
      uint32_t _redis_port
  ) {
    cursor = 0;

    // If we have not attempted to connect + certify, we need to apply our root certificates
    // and open the tcp connection using our `client.`
    if (certified == ECertificationStage::NotRequested) {
      certified = ECertificationStage::CerificationRequested;

#ifndef RELEASE
      log_d("attempting to certify redis connection");
#endif
      extern const uint8_t redis_root_ca[] asm("_binary_embeds_redis_host_root_ca_pem_start");

      client.setCACert((char *) redis_root_ca);

#ifndef RELEASE
      log_d("attempting to establish connection with redis %s:%d", _redis_host, _redis_port);
#endif

      int result = client.connect(_redis_host, _redis_port);

      if (result != 1) {
#ifndef RELEASE
        log_e("unable to establish connection - %d", result);
#endif
        return Manager::EManagerMessage::FailedConnection;
      }

#ifndef RELEASE
      log_d("successfully connected to redis host/port ('%s')", _redis_auth);
#endif

      // At this point we have a valid tcp connection using tls and can start writing our redis
      // commands. Start by sending the `AUTH` command.
      char auth_command [256];
      memset(auth_command, '\0', 256);
      sprintf(auth_command, "*2\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n", strlen(_redis_auth), _redis_auth);
      size_t written = client.print(auth_command);

#ifndef RELEASE
      log_d("wrote %d bytes on first message", written);
#endif
    }

    // Clear out the framebuffer.
    memset(framebuffer, '\0', framebuffer_size);

    // Read everything we can off our client.
    while (client.available() && cursor < framebuffer_size - 1) {
      framebuffer[cursor] = (char) client.read();
      cursor += 1;
    }

    // If we are not certified, assume this is our first `+OK` response pulled in off the 
    // `AUTH` command that was issued previously.
    if (certified != ECertificationStage::Certified) {
#ifndef RELEASE
      log_d("not yet certified with server, skipping pop");
#endif

      // If we had no data, do nothing (we're still waiting for a response).
      if (cursor == 0) {
        return std::nullopt;
      }

      if (strcmp(framebuffer, "+OK\r\n") == 0) {
#ifndef RELEASE
        log_d("successfully authorized connection to redis");
#endif
        certified = ECertificationStage::Certified;

        return Manager::EManagerMessage::EstablishedConnection;
      }

      // We've received some unknown message while not certified, ignore.
      return std::nullopt;
    }

#ifndef RELEASE
    // At this point we've got a connection and have successfully authorized ourselves.
    log_d("writing pop command for next frame");
#endif

    client.print(redis_pop);

    if (strcmp(framebuffer, "$-1\r\n") == 0 || cursor == 0) {
      cursor = 0;
      memset(framebuffer, '\0', framebuffer_size);
      return std::nullopt;
    }

    uint8_t stage = 0, len = 0, size = 0;
    char * n = framebuffer;
    char message [framebuffer_size];
    memset(message, '\0', framebuffer_size);

    while (*n != '\0' && (stage == 0 ? len < 2 : (stage == 1 ? len < 20 : len < size))) {
      char tok = *n;
      n++;

      if (tok == '$' && stage == 0) {
        stage = 1;
        continue;
      }

      // After we've identified ourselves as a bulk string, the first time we see a newline
      // means that we've parsed the size of the message.
      if (stage == 1 && tok == '\r') {
        // Skip the '\n' that will follow
        n++;

        // Parse the contents of our current message as the size.
        for (uint8_t i = 0; i < len; i++) {
          size = (size * 10) + (message[i] - '0');
        }

        // We're now parsing the "actual" content.
        stage = 2;
        len = 0;

#ifndef RELEASE
        log_d("parsed message length - %d (from %s)", size, message);
#endif

        // Clear out our buffer in preparation for the real content.
        memset(message, '\0', framebuffer_size);
        continue;
      }

      message[len] = tok;
      len += 1;
    }

    if (len != size || len == 0 || stage != 2) {
#ifndef RELEASE
      log_e("unable to parse message - '%s'", message);
#endif

      cursor = 0;
      memset(framebuffer, '\0', framebuffer_size);
      return std::nullopt;
    }

#ifndef RELEASE
    log_d("parsed message '%s' (len %d)", message, len);
#endif

    // Place our parsed message into the framebuffer now.
    memcpy(framebuffer, message, len);
    if (len < framebuffer_size) {
      framebuffer[len] = '\0';
    }

    return std::optional(Manager::EManagerMessage::ReceivedMessage);
  }

  bool Manager::Disconnected::update(std::optional<wifimanager::Manager::EManagerMessage> &message) {
    if (message == wifimanager::Manager::EManagerMessage::Connected) {
      return true;
    }

    tick += 1;
    if (tick > 100) {
      tick = 0;
    }

    return false;
  }
}

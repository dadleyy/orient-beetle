#include "redis-manager.hpp"

namespace redismanager {

  Manager::Manager(std::tuple<const char *, const uint32_t, std::pair<const char *, const char *>> config):
    _redis_host(std::get<0>(config)),
    _redis_port(std::get<1>(config)),
    _redis_auth(std::get<2>(config)),
    _paused(false),
    _state(Disconnected { tick: 0 })
  {
  }

  void Manager::begin(void) {
    _preferences.begin("beetle-redis", false);
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
          log_d("attempting to move from disconnect to connected");

          _paused = false;
          _state.emplace<Connected>(&_preferences);
        }
        break;
      }
      case 1:
        Connected * c = std::get_if<1>(&_state);

        // If we've been completely disconnected, restart.
        if (message == wifimanager::Manager::EManagerMessage::Disconnected) {
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
          log_d("wifi connection was interrupted, pausing all requests");

          return std::nullopt;
        }

        // If we're paused and are about to resume, reset us to disconnected. The next frame will bounce us
        // back into connected.
        if (_paused && message == wifimanager::Manager::ConnectionResumed) {
          _state.emplace<Disconnected>();
          return Manager::EManagerMessage::ConnectionLost;
        }

        return c->update(_redis_host, _redis_auth, _redis_port);
    }

    return std::nullopt;
  }

  uint16_t Manager::Connected::copy(char * destination, uint16_t size) {
    if (_cursor == 0 || _certified != ECertificationStage::Identified) {
      return 0;
    }

    uint16_t amount = _cursor < size ? _cursor : size;
    memcpy(destination, _framebuffer, amount);
    _cursor = 0;
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
    return amount;
  }

  std::optional<Manager::EManagerMessage> Manager::Connected::update(
      const char * _redis_host,
      const std::pair<const char *, const char *> & _redis_auth,
      uint32_t _redis_port
  ) {
    _cursor = 0;

    // If we have not attempted to connect + certify, we need to apply our root certificates
    // and open the tcp connection using our `client.`
    if (_certified == ECertificationStage::NotRequested) {
      _certified = ECertificationStage::CerificationRequested;

      log_d("attempting to certify redis connection");

      // TODO: figure out how to maybe read this elsewhere to make this library more portable?
      extern const uint8_t redis_root_ca[] asm("_binary_embeds_redis_host_root_ca_pem_start");

      _client.setCACert((char *) redis_root_ca);
      log_d("attempting to establish connection with redis %s:%d", _redis_host, _redis_port);

      int result = _client.connect(_redis_host, _redis_port);

      if (result != 1) {
        log_e("unable to establish connection - %d", result);
        return Manager::EManagerMessage::FailedConnection;
      }

      size_t stored_id_len = _preferences->getString("device-id", _device_id, MAX_ID_SIZE);

      // If we have a stored device id in our preferences/persistent memory, attempt to authorize
      // with it and move into the authorization requested stage.
      if (stored_id_len > 0) {
        _connected_with_cached_id = true;
        _device_id_len = stored_id_len;

        log_d("has stored device id '%s', trying it out.", _device_id);

        // Now that we have a valid device id, authorize as that.
        char * auth_command = (char *) malloc(sizeof(char) * 256);
        memset(auth_command, '\0', 256);
        sprintf(
          auth_command,
          "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
          strlen(_device_id),
          _device_id,
          strlen(_device_id),
          _device_id
        );
        _client.print(auth_command);
        free(auth_command);

        _certified = ECertificationStage::AuthorizationRequested;
        return Manager::EManagerMessage::IdentificationReceived;
      }

      // At this point we have a valid tcp connection using tls and can start writing our redis
      // commands. Start by sending the `AUTH` command using the device id consumer ACL that should
      // be safe for global use.
      char * auth_command = (char *) malloc(sizeof(char) * 256);
      memset(auth_command, '\0', 256);
      const char * redis_username = std::get<0>(_redis_auth);
      const char * redis_password = std::get<1>(_redis_auth);
      log_d("writing redis auth command %s:%s", redis_username, redis_password);
      sprintf(
        auth_command,
        "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
        strlen(redis_username),
        redis_username,
        strlen(redis_password),
        redis_password
      );
      // TODO: `_client.write(...)` might be a better choice here
      size_t written = _client.print(auth_command);
      free(auth_command);

      log_d("wrote %d bytes on first message", written);

      // Return early, delay read for another frame at least.
      return std::nullopt;
    }

    // If we're officially "certified" with the server, it is time to request our identity.
    if (_certified == ECertificationStage::Certified) {
      _certified = ECertificationStage::IdentificationRequested;
      // At this point we've got a connection and have successfully authorized ourselves.
      log_d("writing pop command for next frame");
      // TODO: `_client.write(...)` might be a better choice here
      _client.print(REDIS_REGISTRATION_POP);

      return std::nullopt;
    }

    // Clear out the framebuffer.
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);

    // Read everything we can off our client.
    while (_client.available() && _cursor < FRAMEBUFFER_SIZE - 1) {
      // TODO: `_client.read((uint8_t *) _framebuffer, FRAMEBUFFER)` might be a better choice here
      _framebuffer[_cursor] = (char) _client.read();
      _cursor += 1;
    }


    // Explicitly handle wrongpass.
    if (strcmp(_framebuffer, WRONG_PASS_ERR) == 0) {
      log_e("wrongpass received, resetting client");
      reset();

      return std::nullopt;
    }

    if (strcmp(_framebuffer, NO_PERM_ERR) == 0) {
      log_e("permissions lost, resetting client");
      reset();

      return std::nullopt;
    }

    if (strcmp(_framebuffer, OK) == 0 && _certified == ECertificationStage::AuthorizationRequested) {
      log_d("received 'OK' during certification stage '%d'", _certified);
      _certified = ECertificationStage::Identified;

      return std::nullopt;
    }

    // If we are not certified, assume this is our first `+OK` response pulled in off the 
    // `AUTH` command that was issued previously.
    if (_certified == ECertificationStage::CerificationRequested) {
      log_d("not yet certified with server, skipping pop (pulled %s)", _framebuffer);

      // If we had no data, do nothing (we're still waiting for a response).
      if (_cursor == 0) {
        return std::nullopt;
      }

      if (strcmp(_framebuffer, OK) == 0) {
        log_d("successfully authorized connection to redis");
        _certified = ECertificationStage::Certified;

        return Manager::EManagerMessage::EstablishedConnection;
      }

      // We've received some unknown message while not certified, ignore.
      return std::nullopt;
    }

    if (_cursor > 0 && _certified == ECertificationStage::Identified) {
      _empty_identified_reads = 0;
    }

    // If we popped something off the queue and it was empty, do nothing.
    if (strcmp(_framebuffer, "$-1\r\n") == 0 || _cursor == 0) {
      if (_certified == ECertificationStage::IdentificationRequested) {
        log_e("unable to pull device id, is registrar running?");
      }

      // If we had nothing but we're identified, write our next pop before moving on.
      if (_certified == ECertificationStage::Identified) {
        // If there was literally no data to read, increase our counter and check to see
        // if we're relying on cached credentials. If so, clear it and restart.
        if (_cursor == 0) {
          _empty_identified_reads += 1;
          log_e("empty reads while identified - %d", _empty_identified_reads);

          if (_empty_identified_reads > 200 && _connected_with_cached_id) {
            reset();

            return std::nullopt;
          }
        }

        write_pop();
      }

      _cursor = 0;
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);

      return std::nullopt;
    }

    // At this point, we have what appears to be a valid message sitting in our framebuffer.
    // Now we will attempt to parse out the actual message contents (later copied back into
    // the framebuffer).
    uint8_t stage = 0, len = 0, size = 0;
    char * n = _framebuffer;
    char message [FRAMEBUFFER_SIZE];
    memset(message, '\0', FRAMEBUFFER_SIZE);
    bool isint = false;

    while (*n != '\0' && (stage == 0 ? len < 2 : (stage == 1 ? len < 20 : len < size))) {
      char tok = *n;
      n++;

      if ((tok == '$' || tok == ':') && stage == 0) {
        stage = 1;

        // Special casing the not-so-valuable integer response. At some point it will be
        // nice to handle this in some way but for the time being it is only valuable for
        // the final registration `rpush` that is issued.
        if (tok == ':') {
          isint = true;
        }

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

        // Clear out our buffer in preparation for the real content.
        memset(message, '\0', FRAMEBUFFER_SIZE);
        continue;
      }

      message[len] = tok;
      len += 1;
    }

    // If we parsed something that wasn't a integer response and it was either empty or
    // the value returned is not the same as our index it does not appear to be a valid
    // message.
    if ((len != size || len == 0 || stage != 2) && !isint) {
      log_e("unable to parse message - '\n%s\n'", _framebuffer);
      _cursor = 0;
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      return std::nullopt;
    }

    // TODO: here we are copying our parsed message back onto the memory we've allocated for
    // our framebuffer. It is definitely possible that this is not necessary.
    if (isint == false) {
      memcpy(_framebuffer, message, len);
      if (len < FRAMEBUFFER_SIZE) {
        _framebuffer[len] = '\0';
      }
    }

    if (_certified == ECertificationStage::IdentificationRequested) {
      log_d("assuming '%s' is our identity", _framebuffer);

      _device_id_len = len < MAX_ID_SIZE ? len : MAX_ID_SIZE;
      memcpy(_device_id, message, _device_id_len);

      size_t stored_id_len = _preferences->putString("device-id", _device_id);
      log_d("stored device id (%d bytes)", stored_id_len);

      // We now have our individual identity.
      _certified = ECertificationStage::Identified;

      log_d("writing auth command with new id %s", _device_id);
      // Now that we have a valid device id, authorize as that.
      char * auth_command = (char *) malloc(sizeof(char) * 256);
      memset(auth_command, '\0', 256);
      sprintf(
        auth_command,
        "*3\r\n$4\r\nAUTH\r\n$%d\r\n%s\r\n$%d\r\n%s\r\n",
        strlen(_device_id),
        _device_id,
        strlen(_device_id),
        _device_id
      );
      _client.print(auth_command);
      free(auth_command);

      _certified = ECertificationStage::AuthorizationRequested;
      return Manager::EManagerMessage::IdentificationReceived;
    }

    if (_certified == ECertificationStage::Identified) {
      write_pop();
    }

    return isint ? std::nullopt : std::optional(Manager::EManagerMessage::ReceivedMessage);
  }

  void Manager::Connected::reset(void) {
    _empty_identified_reads = 0;
    _certified = ECertificationStage::NotRequested;
    _client.stop();
    _preferences->remove("device-id");

    _cursor = 0;
    memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
  }

  Manager::Connected::Connected(Preferences* _preferences):
    _certified(ECertificationStage::NotRequested),
    _cursor(0),
    _write_delay(0),
    _device_id_len(0),
    _empty_identified_reads(0),
    _preferences(_preferences) {
      _framebuffer = (char*) malloc(sizeof(char) * FRAMEBUFFER_SIZE);
      _device_id = (char*) malloc(sizeof(char) * MAX_ID_SIZE);
      memset(_framebuffer, '\0', FRAMEBUFFER_SIZE);
      memset(_device_id, '\0', MAX_ID_SIZE + 1);
  }

  Manager::Connected::~Connected() {
    log_d("cleaning up redis client connection");
    free(_framebuffer);
    free(_device_id);
    _client.stop();
  }

  inline uint16_t Manager::Connected::write_pop(void) {
    // Only actually write a pop every 10 times we request one...
    if (_write_delay++ < 10) {
      return _write_delay == 5 ? write_push() : 0;
    }

    _write_delay = 0;

    char buffer [256];
    memset(buffer, '\0', 256);
    uint8_t keysize = strlen(_device_id) + 3;
    sprintf(buffer, "*2\r\n$4\r\nLPOP\r\n$%d\r\nob:%s\r\n", keysize, _device_id);

    // TODO: `_client.write(...)` might be a better choice here
    return _client.print(buffer);
  }

  inline uint16_t Manager::Connected::write_push(void) {
    char buffer [256];
    memset(buffer, '\0', 256);
    sprintf(buffer, "*3\r\n$5\r\nRPUSH\r\n$4\r\nob:i\r\n$%d\r\n%s\r\n", strlen(_device_id), _device_id);

    // TODO: `_client.write(...)` might be a better choice here
    return _client.print(buffer);
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

  uint8_t Manager::id_size(void) {
    Connected * c = std::get_if<Connected>(&_state);
    if (!c) {
      return 0;
    }
    return c->_device_id_len;
  }

  uint8_t Manager::copy_id(char * dest, uint8_t max) {
    Connected * c = std::get_if<Connected>(&_state);
    if (!c) {
      return 0;
    }
    uint8_t amount = c->_device_id_len < max ? c->_device_id_len : max;
    memcpy(dest, c->_device_id, amount);
    return amount;
  }
}

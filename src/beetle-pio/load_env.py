Import("env")

import os

redis_port = None
redis_host = None

if os.path.exists(".env") and os.path.isfile(".env"):
    print("loading dotenv file")
    env_file = open(".env")
    lines = env_file.readlines()
    for line in lines:
        if line.startswith("REDIS_HOST="):
            redis_host = line.lstrip("REDIS_HOST=\"").strip().rstrip('"')
            print("found redis host - '%s'" % redis_host)
        elif line.startswith("REDIS_PORT="):
            redis_port = line.lstrip("REDIS_PORT=\"").strip().rstrip('"')
            print("found port '%s'" % redis_port)

if "REDIS_PORT" in os.environ:
    redis_port = os.environ["REDIS_PORT"]

if "REDIS_HOST" in os.environ:
    redis_host = os.environ["REDIS_HOST"]

if redis_port is None or redis_host is None:
    raise Exception("Missing 'REDIS_PORT' or 'REDIS_HOST'")

print("environment ready - redis %s:%s" % (redis_host, redis_port))

env.ProcessFlags("-DREDIS_PORT=%s -DREDIS_HOST=\\\"%s\\\"" % (redis_port, redis_host))

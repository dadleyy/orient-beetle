Import("env")

import os

redis_port = None
redis_auth = None
redis_host = None

wifi_ssid = None
wifi_password = None

def get_value(line):
    return line.split("=")[1].lstrip('"').rstrip().rstrip('"')

if os.path.isfile("./embeds/redis_host_root_ca.pem") != True:
    raise Exception("Missing 'redis_host_root_ca.pem' file, please see README.md")

if os.path.exists(".env") and os.path.isfile(".env"):
    print("loading dotenv file")
    env_file = open(".env")
    lines = env_file.readlines()
    for line in lines:
        if line.startswith("WIFI_SSID="):
            wifi_ssid = get_value(line)
        if line.startswith("WIFI_PASSWORD="):
            wifi_password = get_value(line)

        if line.startswith("REDIS_AUTH="):
            redis_auth = get_value(line)
            print("found redis auth - '%s'" % redis_auth)

        if line.startswith("REDIS_HOST="):
            redis_host = get_value(line)
            print("found redis host - '%s'" % redis_host)

        elif line.startswith("REDIS_PORT="):
            redis_port = get_value(line)
            print("found port '%s'" % redis_port)

if "REDIS_PORT" in os.environ:
    redis_port = os.environ["REDIS_PORT"]

if "REDIS_HOST" in os.environ:
    redis_host = os.environ["REDIS_HOST"]

if redis_port is None or redis_host is None:
    raise Exception("Unable to find 'REDIS_HOST' or 'REDIS_PORT' in environment. Please create a '.env' file")

if redis_auth is None:
    raise Exception("Unable to find 'REDIS_AUTH' in environment (or .env file)")

flag_str = "-DWIFI_PASSWORD='\"{wifi_password}\"' \
            -DWIFI_SSID='\"{wifi_ssid}\"' \
            -DREDIS_HOST='\"{redis_host}\"' \
            -DREDIS_PORT='{redis_port}' \
            -DREDIS_AUTH='\"{redis_auth}\"'".format(redis_host = redis_host,
                     redis_port = redis_port,
                     redis_auth = redis_auth,
                     wifi_password = wifi_password,
                     wifi_ssid = wifi_ssid);

print("environment ready, adding definitions to build flags")

if 'UNSAFE_LOGGING' in os.environ:
    print("build flags - %s" % flag_str);

env.ProcessFlags(flag_str)

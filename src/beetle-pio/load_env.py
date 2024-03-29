Import("env")

import os

redis_port = None
redis_host = None
beetle_version = "dev"

redis_auth_username = None
redis_auth_password = None

def get_value(line):
    return line.split("=")[1].lstrip('"').rstrip().rstrip('"')

if os.path.isfile("./embeds/redis_host_root_ca.pem") != True:
    raise Exception("Missing './embeds/redis_host_root_ca.pem' file, please see README.md")

if os.path.exists(".env") and os.path.isfile(".env"):
    print("loading dotenv file")
    env_file = open(".env")
    lines = env_file.readlines()
    for line in lines:
        if line.startswith("REDIS_AUTH_USERNAME="):
            redis_auth_username = get_value(line)

        if line.startswith("REDIS_AUTH_PASSWORD="):
            redis_auth_password = get_value(line)

        if line.startswith("REDIS_HOST="):
            redis_host = get_value(line)
            print("found redis host - '%s'" % redis_host)

        elif line.startswith("REDIS_PORT="):
            redis_port = get_value(line)
            print("found port '%s'" % redis_port)

if "REDIS_AUTH_USERNAME" in os.environ:
    redis_auth_username = os.environ["REDIS_AUTH_USERNAME"]

if "REDIS_AUTH_PASSWORD" in os.environ:
    redis_auth_password = os.environ["REDIS_AUTH_PASSWORD"]

if "BEETLE_VERSION" in os.environ:
    beetle_version = os.environ["BEETLE_VERSION"]

if "REDIS_PORT" in os.environ:
    redis_port = os.environ["REDIS_PORT"]

if "REDIS_HOST" in os.environ:
    redis_host = os.environ["REDIS_HOST"]

if redis_port is None or redis_host is None:
    raise Exception("Unable to find 'REDIS_HOST' or 'REDIS_PORT' in environment. Please create a '.env' file")

if redis_auth_username is None:
    raise Exception("Unable to find 'REDIS_AUTH_USERNAME' in environment (or .env file)")

if redis_auth_password is None:
    raise Exception("Unable to find 'REDIS_AUTH_PASSWORD' in environment (or .env file)")

flag_str = "-DREDIS_HOST='\"{redis_host}\"' \
            -DREDIS_PORT='{redis_port}' \
            -DBEETLE_VERSION='\"{beetle_version}\"' \
            -DREDIS_AUTH_PASSWORD='\"{redis_auth_password}\"' \
            -DREDIS_AUTH_USERNAME='\"{redis_auth_username}\"'".format(redis_host = redis_host,
                     beetle_version = beetle_version,
                     redis_port = redis_port,
                     redis_auth_username = redis_auth_username,
                     redis_auth_password = redis_auth_password);

if 'UNSAFE_LOGGING' in os.environ:
    print("environment loaded, added to build flags %s" % flag_str);

env.ProcessFlags(flag_str)

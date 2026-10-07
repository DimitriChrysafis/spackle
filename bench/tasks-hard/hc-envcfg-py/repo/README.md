# envcfg

Layered configuration: defaults < config file (JSON) < .env file <
process environment. `${VAR}` and `${VAR:-default}` interpolation for
values. Typed getters: `get_int`, `get_bool`, `require`.

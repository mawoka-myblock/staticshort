# StaticShort

## What

A URL shortener with static configuration via environment variables

## Why

I needed a URL shortener, something between a simple redirect in the web interface and a fully fledged one. That's the exact middleground I personally need.


## How
Get the `docker-compose.yml` and configure it to your likings.

You can add as many short url definitions as you want, only make sure that you name them all differently.
```yaml
environment:
    SR_REDIR_test: "/hi,/test,/"
    SR_REDIR_test__TARGET: https://g.co
    SR_REDIR_test__CODE: 307
    SR_REDIR_test__JS_ONLY: false
    SR_REDIR_test__PRESERVE_PARAMS: true
```

In the example, the single handler is named "test".
# sozu-command-futures, a library driving the sōzu HTTP reverse proxy - moved to https://github.com/sozu-proxy/sozu/tree/master/futures

This reverse proxy receives orders on a Unix socket, with a simple request-response
protocol using JSON.
This library wraps calls to that socket through a tokio transport.

See the example for more information.

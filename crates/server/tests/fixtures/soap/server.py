"""Real Spyne SOAP1.1 / SOAP1.2 fixture; no Docker or external target."""
import sys
import six
import collections.abc, http.cookies, urllib.parse, urllib.request, urllib.error
# Spyne's bundled older six importer predates Python3.12. Use the installed mature
# compatibility library through module aliases; SOAP/XSD code remains upstream.
sys.modules.update({
    'spyne.util.six': six,
    'spyne.util.six.moves': six.moves,
    'spyne.util.six.moves.collections_abc': collections.abc,
    'spyne.util.six.moves.http_cookies': http.cookies,
    'spyne.util.six.moves.urllib': six.moves.urllib,
    'spyne.util.six.moves.urllib.parse': urllib.parse,
    'spyne.util.six.moves.urllib.request': urllib.request,
    'spyne.util.six.moves.urllib.error': urllib.error,
})
from spyne import Application, ServiceBase, Unicode, Integer, Array, ComplexModel, rpc, Fault
from spyne.util import six
if not hasattr(six, "get_function_name"):
    six.get_function_name = lambda f: f.__name__
from spyne.protocol.soap import Soap11, Soap12
from spyne.server.wsgi import WsgiApplication
from wsgiref.simple_server import make_server
import argparse

class Item(ComplexModel):
    label = Unicode(min_occurs=0)
    count = Integer

class Service(ServiceBase):
    @rpc(Unicode, Array(Item), _returns=Unicode)
    def Echo(ctx, name, items):
        if name == 'fault':
            raise Fault(faultcode='Client.Invalid', faultstring='Fixture rejected name', detail={'message': 'fixture detail'})
        return 'Hello ' + (name or '')

p = argparse.ArgumentParser()
p.add_argument('--port', type=int, default=18897)
p.add_argument('--version', choices=['1.1','1.2'], default='1.1')
a=p.parse_args()
protocol = Soap11 if a.version == '1.1' else Soap12
application = WsgiApplication(Application([Service], tns='urn:moleapi:soap', in_protocol=protocol(validator='lxml'), out_protocol=protocol()))
# Fixture demonstrates ordinary HTTP authentication independently of WSDL discovery.
def app(env, start):
    if env.get('REQUEST_METHOD') == 'POST' and env.get('HTTP_AUTHORIZATION') != 'Bearer fixture-token':
        start('401 Unauthorized', [('Content-Type','text/plain')])
        return [b'Fixture bearer token required']
    return application(env, start)
print(f'Spyne SOAP{a.version} listening on 127.0.0.1:{a.port}', flush=True)
make_server('127.0.0.1',a.port,app).serve_forever()

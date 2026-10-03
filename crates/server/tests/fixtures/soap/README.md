Real SOAP fixture: Spyne 2.14.0 (LGPL-2.1), lxml 6.0.2 (BSD-3-Clause).
Install in a disposable venv, then run `python server.py --port 18897 --version 1.1`
and `python server.py --port 18898 --version 1.2`. POST requires bearer fixture-token.
`/?wsdl` provides generated real WSDL/XSD. `Echo` accepts name and repeated optional Item values;
name=fault returns the protocol's actual SOAP Fault / HTTP500. No containers/services required.

Reproduce with `python3 -m venv /tmp/moleapi-soap-fixture` and
`/tmp/moleapi-soap-fixture/bin/pip install -r requirements.txt`. The fixture aliases
Spyne's old bundled six imports to installed six1.17 for current Python; SOAP/XSD
implementation remains Spyne/lxml. Python3.13+ gets the maintained legacy-cgi module.
Golden spyne11/12.wsdl were fetched from these actual services. entry.wsdl,
bindings.wsdl and types.xsd are lxml-generated virtual import variants of spyne11.
Run the real service test with
`cargo test -p moleapi-server --test soap --no-default-features real_spyne -- --ignored`.

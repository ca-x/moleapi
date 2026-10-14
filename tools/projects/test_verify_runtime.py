"""Checks standard chunked framing used by the generated-client wire fixture."""
import importlib.util
import io
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("verify_runtime", Path(__file__).with_name("verify-runtime.py"))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)

class ChunkedFixtureTests(unittest.TestCase):
    def test_binary_chunks_extensions_trailers_and_stream_ownership(self):
        stream = io.BytesIO(b'3;example=yes\r\n\x00\xffA\r\n2\r\n\r\n\r\n0\r\nX-Receipt: done\r\n\r\nremaining')
        self.assertEqual(runtime.read_chunked_body(stream), b'\x00\xffA\r\n')
        self.assertEqual(stream.read(), b'remaining')

    def test_incomplete_chunk_is_not_accepted_as_success(self):
        with self.assertRaises(runtime.http.client.IncompleteRead):
            runtime.read_chunked_body(io.BytesIO(b'8\r\nshort'))

if __name__ == "__main__":
    unittest.main()

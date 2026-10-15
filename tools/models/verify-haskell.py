#!/usr/bin/env python3
"""Compile/run original Haskell models and typed Aeson errors using an explicit GHC."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import shutil

spec = importlib.util.spec_from_file_location('native_models_qa', Path(__file__).with_name('verify-native.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)

HASKELL = r'''{-# LANGUAGE OverloadedStrings #-}
module Main where
import QuickType
import Data.Aeson (eitherDecode, encode)
import qualified Data.ByteString.Lazy.Char8 as B
import Control.Monad (unless)
check :: Bool -> IO ()
check value = unless value (fail "Native model fields lost")
reject :: B.ByteString -> IO ()
reject source = case (eitherDecode source :: Either String Pet) of
  Left _ -> pure ()
  Right _ -> fail "Malformed model accepted"
main :: IO ()
main = do
  source <- B.readFile "sample.json"
  decoded <- either fail pure (eitherDecode source :: Either String Pet)
  check (petIDPet decoded == 0 && notePet decoded == Nothing)
  check (case statusPet decoded of ReadyStatus -> True; _ -> False)
  case friendPet decoded of
    Nothing -> fail "Recursive native model lost"
    Just child -> do
      check (petIDPet child == 2 && notePet child == Just "鼹鼠 \"quotes\" & +")
      check (case statusPet child of DoneStatus -> True; _ -> False)
  B.putStrLn (encode decoded)
  let child = Pet {friendPet = Nothing, petIDPet = 2, notePet = Just "鼹鼠 \"quotes\" & +", statusPet = DoneStatus}
      built = Pet {friendPet = Just child, petIDPet = 0, notePet = Nothing, statusPet = ReadyStatus}
  B.putStrLn (encode built)
  __PUBLIC_DECODER__
  __MULTI__
  reject "{\"id\":0,\"status\":\"invalid\"}"
  reject "\"not-an-object\""
  reject "{\"status\":\"ready\"}"
  reject "{\"id\":0,\"status\":\"ready\",\"friend\":{\"id\":2,\"status\":\"invalid\"}}"
'''

MULTI = r'''  case decodeEmpty "{}" of Nothing -> fail "Empty decoder failed"; Just _ -> pure ()
  check (case decodeEmpty "\"wrong\"" of Nothing -> True; Just _ -> False)
  let arrayInput = B.concat ["[", source, ",", source, "]"]
      mapInput = B.concat ["{\"root\":", source, "}"]
  values <- maybe (fail "Array alias decode failed") pure (decodePetArray arrayInput)
  check (length values == 2 && petArrayIDPetArrayElement (head values) == 0)
  check (case statusPetArrayElement (head values) of ReadyStatus -> True; _ -> False)
  mapping <- maybe (fail "Map alias decode failed") pure (decodePetMap mapInput)
  check (HashMap.size mapping == 1)
  item <- maybe (fail "Map entry lost") pure (HashMap.lookup "root" mapping)
  check (moleapiModelsJSONIDMoleapiModelsJSON item == 0)
  case (eitherDecode "[]" :: Either String Empty) of Left _ -> pure (); Right _ -> fail "Empty object accepted array"
'''


def verify(args):
    assert args.fixtures.is_absolute() and args.fixtures.is_dir()
    assert args.ghc.is_absolute() and args.ghc.is_file()
    root = args.fixtures / 'model-haskell'
    manifest = json.loads((root / 'moleapi-generation.json').read_text())
    assert manifest['target'] == 'model-haskell' and manifest['engine'] == 'quicktype-core@26.0.0+moleapi.1'
    source = (root / 'fixture-source.json').read_bytes()
    assert json.loads(source)['openapi'] == args.dialect
    assert hashlib.sha256(source).hexdigest() == manifest['source_sha256']
    before = {file['path']: hashlib.sha256((root / file['path']).read_bytes()).hexdigest() for file in manifest['files']}
    assert all(before[file['path']] == file['sha256'] for file in manifest['files'])
    consumer = args.fixtures / 'model-haskell-consumer'
    consumer.mkdir(exist_ok=False)
    shutil.copyfile(root / 'models.haskell', consumer / 'QuickType.hs')
    multi = 'Empty' in json.loads(source)['components']['schemas']
    public = 'decodePet' if multi else 'decodeTopLevel'
    driver = HASKELL.replace('__PUBLIC_DECODER__', f'case {public} source of Nothing -> fail "Public decoder failed"; Just value -> check (petIDPet value == 0)').replace('  __MULTI__', MULTI if multi else '')
    if multi:
        driver = driver.replace('import Control.Monad', 'import qualified Data.HashMap.Strict as HashMap\nimport Control.Monad')
    (consumer / 'Main.hs').write_text(driver)
    (consumer / 'sample.json').write_text(json.dumps(native.SAMPLE, ensure_ascii=False) + '\n')
    native.run([str(args.ghc), '-O2', '-dynamic', '-package', 'aeson', '-package', 'unordered-containers', 'Main.hs', '-o', 'model-qa'], consumer)
    output = native.run([str(consumer / 'model-qa')], consumer)
    native.check(output)
    assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
    evidence = {'target': 'model-haskell', 'dialect': args.dialect, 'engine': manifest['engine'], 'platform': platform.platform(),
                'ghc': native.run([str(args.ghc), '--version'], consumer).strip(), 'source_sha256': manifest['source_sha256'],
                'generated_source_sha256': before, 'profiles': ['decoded', 'constructed', 'recursive', 'zero', 'enum', 'null', 'unicode', 'typed-invalid-enum', 'typed-invalid-object', 'typed-missing-required', 'typed-invalid-nested-enum'], 'result': 'passed'}
    if multi:
        evidence['profiles'] += ['distinct-root-decoders', 'native-array-alias', 'native-map-alias', 'empty-object', 'typed-empty-object-rejection']
    (consumer / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'PASS Haskell/{args.dialect}: original native models and typed Aeson errors')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--dialect', required=True, choices=['3.0.3', '3.1.0'])
    parser.add_argument('--ghc', required=True, type=Path)
    verify(parser.parse_args())

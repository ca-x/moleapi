"""Compile/run trusted generated model fixtures; never execute imported code.

First run the fixture_models Cargo example with an explicit application worker.
Then run this script with --fixtures, --dotnet and --typescript paths.
Requires Python, Node, Go and the explicitly selected .NET/TypeScript toolchains.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--fixtures', required=True, type=Path)
parser.add_argument('--dotnet', required=True, type=Path)
parser.add_argument('--typescript', required=True, type=Path)
args = parser.parse_args()
sample = {'id': 1, 'status': 'ready', 'note': None, 'friend': {'id': 2, 'status': 'done', 'note': '鼹鼠'}}

def run(command, directory, env=None):
    result = subprocess.run(command, cwd=directory, env=env, text=True, capture_output=True, timeout=120)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout

def check(name, output):
    result = json.loads(output[output.index('{'):])
    assert result['id'] == 1 and result['status'] == 'ready', result
    assert result.get('note') is None, result
    assert result['friend']['id'] == 2 and result['friend']['status'] == 'done', result
    assert result['friend']['note'] == '鼹鼠', result
    print(f'PASS {name}: compiled model recursive/enum/nullable/Unicode roundtrip')

with tempfile.TemporaryDirectory(prefix='moleapi-model-runtime-') as temporary:
    root = Path(temporary)
    for target in ['model-python', 'model-javascript', 'model-typescript', 'model-go', 'model-cs']:
        shutil.copytree(args.fixtures / target, root / target)
        (root / target / 'sample.json').write_text(json.dumps(sample), encoding='utf8')
    python = root / 'model-python'
    (python / 'verify.py').write_text("import json\nfrom models import pet_from_dict,pet_to_dict\nprint(json.dumps(pet_to_dict(pet_from_dict(json.load(open('sample.json'))))))\n")
    check('Python', run([sys.executable, 'verify.py'], python))
    javascript = root / 'model-javascript'
    (javascript / 'verify.cjs').write_text("const {toPet,petToJson}=require('./models.js');console.log(petToJson(toPet(require('fs').readFileSync('sample.json','utf8'))));\n")
    check('JavaScript', run(['node', 'verify.cjs'], javascript))
    typescript = root / 'model-typescript'
    run(['node', str(args.typescript), 'models.ts', '--target', 'ES2022', '--module', 'commonjs', '--outDir', 'compiled', '--strict'], typescript)
    (typescript / 'verify.cjs').write_text("const {Convert}=require('./compiled/models.js');console.log(Convert.petToJson(Convert.toPet(require('fs').readFileSync('sample.json','utf8'))));\n")
    check('TypeScript', run(['node', 'verify.cjs'], typescript))
    go = root / 'model-go'
    (go / 'main.go').write_text('package main\nimport("os";"fmt")\nfunc main(){data,e:=os.ReadFile("sample.json");if e!=nil{panic(e)};pet,e:=UnmarshalPet(data);if e!=nil{panic(e)};out,e:=pet.Marshal();if e!=nil{panic(e)};fmt.Println(string(out))}\n')
    check('Go', run(['go', 'run', 'models.go', 'main.go'], go))
    csharp = root / 'model-cs'
    (csharp / 'fixture.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings></PropertyGroup></Project>')
    (csharp / 'Program.cs').write_text('using QuickType;\nConsole.WriteLine(Pet.FromJson(File.ReadAllText("sample.json")).ToJson());\n')
    check('C#', run([str(args.dotnet), 'run', '--project', 'fixture.csproj', '--verbosity', 'quiet'], csharp, {**os.environ, 'DOTNET_SKIP_FIRST_TIME_EXPERIENCE':'1', 'DOTNET_NOLOGO':'1', 'DOTNET_CLI_TELEMETRY_OPTOUT':'1'}))

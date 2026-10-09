"""Fixed command inputs shared by producers and offline receipt consumers."""
THREADS = ['OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS',
           'RAYON_NUM_THREADS', 'VECLIB_MAXIMUM_THREADS', 'NUMEXPR_NUM_THREADS']
PEER_URL = 'https://github.com/haoliri0/SOFT.git'
HOST_SCRIPT = "import platform,os,json,subprocess; print(json.dumps({'uname':list(platform.uname()),'python':platform.python_version(),'cpu_count':os.cpu_count(),'cpu_brand':subprocess.check_output(['lscpu'],text=True),'affinity':sorted(os.sched_getaffinity(0)),'thread_environment':{k:os.environ.get(k) for k in ['OMP_NUM_THREADS','OPENBLAS_NUM_THREADS','MKL_NUM_THREADS','RAYON_NUM_THREADS','VECLIB_MAXIMUM_THREADS','NUMEXPR_NUM_THREADS']}}))"


def package_script(names):
    return ("import importlib.metadata as m,pathlib,json,hashlib;names=" + repr(names) + ";"
            "print(json.dumps({n:{'version':m.version(n),'files':{str(pathlib.Path(m.distribution(n).locate_file(f)).resolve()):"
            "hashlib.sha256(pathlib.Path(m.distribution(n).locate_file(f)).read_bytes()).hexdigest() for f in m.distribution(n).files "
            "if str(f).endswith(('.so','.pyd','.py','.json'))}} for n in names}))")


def setup_commands(original, repository, python, revision, native_wheel):
    clifft, symft = (str(original/b/'bin/python') for b in ['clifft','symft'])
    wheels = str(original/'wheels')
    commands = {
        'gcc-version':['gcc','--version'], 'g++-version':['g++','--version'],
        'peer-official-refs':['git','ls-remote',PEER_URL,'refs/heads/main','refs/heads/symft-26-10-08'],
        'peer-clone':['git','clone','--no-checkout',PEER_URL,str(original/'source')],
        'peer-checkout':['git','checkout','--detach',revision],
        'peer-git-archive':['git','archive',revision],
        'clifft-venv':[python,'-m','venv',str(original/'clifft')],
        'symft-venv':[python,'-m','venv',str(original/'symft')],
        'pinned-wheel-download':[symft,'-I','-m','pip','download','--no-cache-dir','--only-binary=:all:','--no-deps','--dest',wheels,'numpy==2.4.6','clifft==0.11.0','setuptools==84.0.0','wheel==0.48.0'],
        'clifft-pinned-install':[clifft,'-I','-m','pip','install','--no-index','--no-deps','--find-links',wheels,'numpy==2.4.6','clifft==0.11.0'],
        'symft-pinned-install':[symft,'-I','-m','pip','install','--no-index','--no-deps','--find-links',wheels,'numpy==2.4.6','setuptools==84.0.0','wheel==0.48.0'],
        'native-symft-wheel':[symft,'-I','-m','pip','wheel','--verbose','--no-cache-dir','--no-build-isolation','--no-deps','--wheel-dir',wheels,str(original/'source/python')],
        'native-symft-install':[symft,'-I','-m','pip','install','--no-index','--no-deps',str(original/'wheels'/native_wheel)],
    }
    for backend, executable in [('clifft',clifft),('symft',symft)]:
        commands[backend+'-package-inspection'] = [executable,'-I','-c',package_script([backend,'numpy'])]
        commands[backend+'-import-inspection'] = [executable,'-I',str(repository/'benchmarks/near_clifford/compiled_sota/worker.py'),backend,str(repository/'benchmarks/near_clifford/compiled_sota/manifest.json'),'1','--mode','identity']
    return commands

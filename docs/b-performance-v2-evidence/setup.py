from pathlib import Path
import subprocess, tarfile, io, shutil, json, hashlib
root=Path('/Users/yuzhe/worktrees/minipy-performance-v2')
out=Path(__file__).parent
sha='564381d304be69d1fd870d4e92530c8024b7c394'
archive=subprocess.check_output(['git','archive',sha],cwd=root)
for name in ['baseline','diagnostics','owner']:
    dest=out/name; dest.mkdir(exist_ok=True)
    with tarfile.open(fileobj=io.BytesIO(archive)) as t: t.extractall(dest,filter='data')
    if name!='baseline':
        for p in ['Cargo.toml','src/codegen.rs','src/bin/minipy.rs','tests/boundaries.rs']:
            shutil.copy2(root/p,dest/p)
    if name=='owner':
        for p in ['src/buffer.rs','src/provider.rs','tests/acceptance.rs']:
            path=dest/p
            text=path.read_text().replace('Weak<[Quote]>','Weak<Box<[Quote]>>')
            if p=='src/buffer.rs':
                text=text.replace('Arc<[Quote]>','Arc<Box<[Quote]>>').replace('Arc::from(quotes.into_boxed_slice())','Arc::new(quotes.into_boxed_slice())')
            path.write_text(text)
    manifest=dest/'bench/Cargo.toml'
    # Unchanged benchmark lane code, snapshot only. Each binary saved before next build.
    subprocess.run(['cargo','+1.96.0','build','--release','--locked','--manifest-path',str(manifest),'--target-dir',str(out/'target')],check=True,cwd=dest)
    shutil.copy2(out/'target/release/quote-bench',out/f'generic-{name}')
identity={'baseline':sha,'rustc':subprocess.check_output(['rustup','run','1.96.0','rustc','--version'],text=True).strip(),'hardware':subprocess.check_output(['sysctl','hw.model','hw.memsize','machdep.cpu.brand_string'],text=True).strip(),'platform':subprocess.check_output(['sw_vers'],text=True).strip()}
identity['source_sha256']={str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for name in ['baseline','diagnostics','owner'] for p in sorted((out/name/'src').rglob('*.rs'))}
(out/'identity.json').write_text(json.dumps(identity,indent=2)+'\n')

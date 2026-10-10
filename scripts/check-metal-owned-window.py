#!/usr/bin/env python3
"""Bounded own-window Cocoa/WindowServer diagnostic; does not replace WSI proof."""
import argparse, hashlib, json, pathlib, struct, subprocess, sys, time, zlib

def rgb_png(path):
    data=path.read_bytes()
    if len(data)>16*1024*1024 or data[:8]!=b"\x89PNG\r\n\x1a\n":raise ValueError("missing/invalid bounded PNG")
    offset=8;packed=b"";header=None
    while offset<len(data):
        n=struct.unpack_from(">I",data,offset)[0];kind=data[offset+4:offset+8];body=data[offset+8:offset+8+n]
        if offset+12+n>len(data) or zlib.crc32(kind+body)!=struct.unpack_from(">I",data,offset+8+n)[0]:raise ValueError("invalid PNG chunk/CRC")
        if kind==b"IHDR":header=struct.unpack(">IIBBBBB",body)
        if kind==b"IDAT":packed+=body
        offset+=12+n
        if kind==b"IEND":break
    if not header:raise ValueError("missing PNG header")
    w,h,depth,color,compression,filtering,interlace=header
    if not(4<=w<=2048 and 4<=h<=2048 and depth==8 and color in(2,6) and compression==0 and filtering==0 and interlace==0):raise ValueError("unsupported bounded capture PNG encoding")
    channels=3 if color==2 else 4;stride=w*channels;limit=(stride+1)*h
    decoder=zlib.decompressobj();raw=decoder.decompress(packed,limit+1)
    if len(raw)!=limit or decoder.unconsumed_tail or not decoder.eof:raise ValueError("invalid PNG inflated extent")
    prior=bytearray(stride);rows=[];at=0
    for _ in range(h):
        f=raw[at];at+=1;row=bytearray(raw[at:at+stride]);at+=stride
        if f>4:raise ValueError("invalid PNG row filter")
        for i in range(stride):
            a=row[i-channels] if i>=channels else 0;b=prior[i];c=prior[i-channels] if i>=channels else 0
            if f==1:v=a
            elif f==2:v=b
            elif f==3:v=(a+b)//2
            elif f==4:
                p=a+b-c;pa,pb,pc=abs(p-a),abs(p-b),abs(p-c);v=a if pa<=pb and pa<=pc else b if pb<=pc else c
            else:v=0
            row[i]=(row[i]+v)&255
        rows.append(row);prior=row
    points=[(w//2,h//2),(w//4,h//2),(3*w//4,h//2),(w//2,3*h//4)]
    samples=[list(rows[y][x*channels:x*channels+3]) for x,y in points]
    return {"width":w,"height":h,"points":points,"rgb":samples,"visible_clear":all(p==[20,51,89] for p in samples)}

def self_test():
    import tempfile
    def png(rgb):
        def chunk(kind,body):return struct.pack(">I",len(body))+kind+body+struct.pack(">I",zlib.crc32(kind+body))
        row=bytes(rgb)*8
        return b"\x89PNG\r\n\x1a\n"+chunk(b"IHDR",struct.pack(">IIBBBBB",8,8,8,2,0,0,0))+chunk(b"IDAT",zlib.compress((b"\0"+row)*8))+chunk(b"IEND",b"")
    with tempfile.TemporaryDirectory(prefix="metal-owned-png-") as root:
        p=pathlib.Path(root)/"image.png";p.write_bytes(png([20,51,89]));assert rgb_png(p)["visible_clear"]
        controls=0
        for value in (b"",png([0,0,0]),png([255,255,255]),png([89,51,20]),png([20,51,89])[:-5]):
            p.write_bytes(value)
            try:assert not rgb_png(p)["visible_clear"]
            except ValueError:pass
            controls+=1
    return {"passed":True,"negative_controls":controls,"native_execution_claim":False}

def capture_failure_self_test():
    """Real bounded child completion despite simulated diagnostic refusal."""
    import contextlib, io, tempfile
    from unittest import mock
    with tempfile.TemporaryDirectory(prefix="metal-capture-refusal-") as root:
        root=pathlib.Path(root);binary=root/"probe"
        binary.write_text("#!/usr/bin/env python3\nimport json,pathlib,sys,time\np=pathlib.Path(sys.argv[2].split('=',1)[1]);p.write_text(json.dumps({'window_id':123,'submitted':1,'completed_inflight':1}))\ntime.sleep(.15)\np.with_suffix('.completed').write_text('completed')\n")
        binary.chmod(0o700)
        cases=[
            ("unsupported-help",[subprocess.CompletedProcess([],0,"","")]),
            ("permission-refusal",[subprocess.CompletedProcess([],0,"-l window -o", ""),subprocess.CompletedProcess([],1,b"",b"permission refused")]),
            ("capture-timeout",[subprocess.CompletedProcess([],0,"-l window -o", ""),subprocess.TimeoutExpired(["screencapture"],2,output=b"partial",stderr=b"denied")]),
        ]
        for name,results in cases:
            output=root/name
            with mock.patch.object(sys,"argv",[__file__,str(binary),str(output),"--source-revision=diagnostic-only"]),mock.patch.object(sys,"platform","darwin"),mock.patch.object(subprocess,"run",side_effect=results),contextlib.redirect_stdout(io.StringIO()):
                assert main()==1
            result=json.loads((output/"result.json").read_text())
            assert result["presentation_passed"] and result["presentation_returncode"]==0
            assert not result["passed"] and not result.get("controlled_termination",False)
            assert (output/"own-window.completed").read_text()=="completed"
    return {"passed":True,"capture_failure_controls":len(cases),"actual_child_completion":True,"native_execution_claim":False}

def main():
    if sys.argv[1:]==["--self-test"]:
        print(json.dumps(self_test(),indent=2));return 0
    if sys.argv[1:]==["--capture-failure-self-test"]:
        print(json.dumps(capture_failure_self_test(),indent=2));return 0
    parser=argparse.ArgumentParser();parser.add_argument("binary",type=pathlib.Path);parser.add_argument("output",type=pathlib.Path);parser.add_argument("--source-revision",required=True);args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    if any(args.output.iterdir()):raise ValueError("fresh diagnostic directory required")
    report={"passed":False,"presentation_passed":False,"physical_mac_claim":False,"source_revision":args.source_revision,"binary_sha256":hashlib.sha256(args.binary.read_bytes()).hexdigest()}
    proc=None;started=time.monotonic();native_started=None;log=None
    try:
        if sys.platform!="darwin":raise RuntimeError("own Cocoa capture requires actual macOS")
        # Native presentation acceptance runs independently of diagnostic
        # capture support/permissions. Start it before any capture CLI calls.
        identity=args.output/"own-window.json";log=(args.output/"presentation.log").open("wb")
        command=[str(args.binary.resolve()),"--automatic","--owned-window-id="+str(identity.resolve())];report["presentation_command"]=command
        native_started=time.monotonic();proc=subprocess.Popen(command,stdout=log,stderr=subprocess.STDOUT)
        capture=pathlib.Path("/usr/sbin/screencapture")
        help_result=subprocess.run([str(capture),"-h"],capture_output=True,text=True,timeout=2)
        help_text=help_result.stdout+help_result.stderr;(args.output/"screencapture-help.txt").write_text(help_text)
        if "-l" not in help_text or "window" not in help_text.lower() or "-o" not in help_text:raise RuntimeError("actual screencapture CLI lacks own-window/shadow controls")
        snapshot=None
        while time.monotonic()-native_started<8 and proc.poll() is None:
            if identity.is_file():
                try:
                    candidate=json.loads(identity.read_text())
                    if candidate.get("submitted",0)>0 and (candidate.get("completed_inflight",0)>0 or candidate.get("retired",0)>0):snapshot=candidate;break
                except (ValueError,OSError):pass
            time.sleep(.02)
        if snapshot is None:raise RuntimeError("no own native window with actual completed GPU command")
        window=snapshot.get("window_id")
        if type(window)!=int or not 0<window<2**32:raise RuntimeError("invalid own NSWindow ID")
        report["native_snapshot"]=snapshot
        image=args.output/"own-window.png";command=[str(capture),"-x","-o","-l"+str(window),str(image.resolve())];report["capture_command"]=command
        result=subprocess.run(command,capture_output=True,timeout=2);report["capture_returncode"]=result.returncode
        (args.output/"capture.stdout").write_bytes(result.stdout);(args.output/"capture.stderr").write_bytes(result.stderr)
        if result.returncode:raise RuntimeError("own-window capture permission/unsupported refusal")
        report["capture"]=rgb_png(image);report["image_sha256"]=hashlib.sha256(image.read_bytes()).hexdigest()
    except Exception as error:
        report["error"]=repr(error)
        if isinstance(error,subprocess.TimeoutExpired):
            report["timeout_command"]=error.cmd
            (args.output/"timeout.stdout").write_bytes(error.stdout or b"");(args.output/"timeout.stderr").write_bytes(error.stderr or b"")
    finally:
        if proc is not None:
            # A capture failure never short-circuits the independent 15-second
            # native gate. Only the existing outer cleanup bound may terminate.
            try:
                proc.wait(timeout=max(.1,17-(time.monotonic()-native_started)))
                report["presentation_passed"]=proc.returncode==0
            except subprocess.TimeoutExpired as error:
                report["presentation_wait_error"]=repr(error)
            if proc.poll() is None:
                proc.terminate()
                try:proc.wait(timeout=1)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    try:proc.wait(timeout=1)
                    except subprocess.TimeoutExpired:report["cleanup_unresolved"]=True
                report["controlled_termination"]=True
            report["presentation_returncode"]=proc.returncode
        if log:log.close()
        report["passed"]=report["presentation_passed"] and report.get("capture",{}).get("visible_clear",False)
        report["elapsed_seconds"]=time.monotonic()-started
        (args.output/"result.json").write_text(json.dumps(report,indent=2)+"\n")
    print(json.dumps(report,indent=2));return 0 if report["passed"] else 1
if __name__=="__main__":sys.exit(main())

"""Bind a reviewed private root witness to its exact ordinary actor and release images."""
import ast
import base64
import hashlib
import re

ROOT_BIN="/usr/libexec/process-tree-guard-root"
CLIENT_BIN="/usr/libexec/process-tree-guard-client"

def digest(raw):
    return hashlib.sha256(raw).hexdigest()

def literal(tree,name):
    node,=[entry.value for entry in tree.body if isinstance(entry,ast.Assign)
           and any(isinstance(t,ast.Name) and t.id==name for t in entry.targets)]
    return ast.literal_eval(node)

def installed_pins(tree):
    candidates=[]
    for entry in ast.walk(tree):
        if not isinstance(entry,ast.For) or not isinstance(entry.target,ast.Tuple):
            continue
        names=[v.id for v in entry.target.elts if isinstance(v,ast.Name)]
        if names==["path","want"]:
            values=ast.literal_eval(entry.iter)
            if [p for p,_ in values]==[ROOT_BIN,CLIENT_BIN]:
                candidates.append(dict(values))
    result,=candidates
    if any(not re.fullmatch(r"[0-9a-f]{64}",pin) for pin in result.values()):
        raise ValueError("invalid installed ELF pin")
    return result

def replace_once(raw,old,new):
    if raw.count(old)!=1:
        raise ValueError("ambiguous pinned source literal")
    return raw.replace(old,new)

def bind(template,ordinary,verified_release_bytes):
    parsed=ast.parse(template)
    previous=literal(parsed,"ORDINARY_B64")
    previous_digest=literal(parsed,"ORDINARY_SHA")
    if digest(base64.b64decode(previous,validate=True))!=previous_digest:
        raise ValueError("input witness embeds inconsistent ordinary authority")
    if set(verified_release_bytes)!={ROOT_BIN,CLIENT_BIN}:
        raise ValueError("expected two exact release roles")
    pins={p:digest(raw) for p,raw in verified_release_bytes.items()}
    updated=replace_once(template,previous.encode(),base64.b64encode(ordinary))
    updated=replace_once(updated,previous_digest.encode(),digest(ordinary).encode())
    for path,old in installed_pins(parsed).items():
        updated=replace_once(updated,old.encode(),pins[path].encode())
    final=ast.parse(updated)
    if installed_pins(final)!=pins or literal(final,"ORDINARY_SHA")!=digest(ordinary):
        raise ValueError("staged observer pins differ from release")
    if base64.b64decode(literal(final,"ORDINARY_B64"),validate=True)!=ordinary:
        raise ValueError("staged ordinary bytes differ from packaged role")
    return updated

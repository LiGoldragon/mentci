# Dolt & DoltHub Workflows

Dolt is a MySQL-compatible SQL database with Git semantics: you can `clone`,
`branch`, `commit`, `merge`, `push`, and `pull` tables the way Git treats files.
DoltHub is the hosted index (like GitHub); DoltLab is the self-hosted option;
Hosted Dolt runs Dolt as a server for you. Licensed Apache 2.0.

This doc captures the workflows that matter for `mentci-next`: using Dolt as
versioned data / agent memory alongside source code in the same jj + git tree.

## Identity

Dolt keeps its own identity, mirroring Git:

```
dolt config --global --add user.name  "Li"
dolt config --global --add user.email "ligoldragon@gmail.com"
```

## Two interfaces for every operation

Every Dolt operation exists in two forms. Same semantics, pick by context.

- **CLI** — for local repo ops, scripting, CI. Looks like `git`.
- **SQL** — `dolt_*` stored procedures and system tables. For use from an app
  or from `dolt sql-server` sessions where you never leave the DB.

| Operation | CLI | SQL |
|---|---|---|
| Init repo | `dolt init` | — |
| Status | `dolt status` | `select * from dolt_status` |
| Stage | `dolt add <table>` | `call dolt_add('<table>')` |
| Commit | `dolt commit -m "msg"` | `call dolt_commit('-am', 'msg')` |
| Log | `dolt log` | `select * from dolt_log` |
| Diff | `dolt diff [branch]` | `select * from dolt_diff_<table>` |
| Branch (list) | `dolt branch` | `select * from dolt_branches` |
| Branch (create) | `dolt branch -b <name>` | `call dolt_checkout('-b','<name>')` |
| Checkout | `dolt checkout <name>` | `call dolt_checkout('<name>')` |
| Merge | `dolt merge <name>` | `call dolt_merge('<name>')` |
| Reset | `dolt reset --hard` | `call dolt_reset('--hard')` |
| Clone | `dolt clone <url>` | `call dolt_clone('<url>')` |
| Push | `dolt push <remote> <br>` | `call dolt_push('<remote>','<br>')` |
| Pull | `dolt pull <remote> <br>` | `call dolt_pull('<remote>','<br>')` |

## Workflow 1 — solo local versioned DB

Use when the DB is a side-effect of dev work and you want checkpoints you
can diff against.

```bash
dolt init
dolt table import --create-table --pk id thoughts thoughts.csv
dolt add thoughts
dolt commit -m "seed initial thoughts"

# work…
dolt sql -q "insert into thoughts values (42, 'it types-all-the-way-down')"
dolt diff                                   # see the pending change
dolt commit -am "add thought 42"
dolt log
```

Rollback discards uncommitted work:

```bash
dolt checkout thoughts    # drop working changes to one table
dolt reset --hard         # drop everything back to HEAD
```

## Workflow 2 — branch, experiment, merge

Branches are cheap; use them to probe a schema change or a batch import
without touching `main`.

```bash
dolt branch -b schema-v2
dolt checkout schema-v2
dolt sql -q "alter table thoughts add column tag varchar(64)"
dolt commit -am "v2: add tag column"

dolt checkout main
dolt merge schema-v2
dolt branch -d schema-v2
```

Conflicts land in per-table system tables (`dolt_conflicts_<table>`); resolve
them by `select`ing and then `delete`ing the rows that represent your choice,
then commit.

## Workflow 3 — collaborate via DoltHub / DoltLab

DoltHub = shared remote. Works like GitHub pull requests, but for data.

```bash
dolt remote add origin https://doltremoteapi.dolthub.com/<org>/<db>
dolt push origin main

# someone else
dolt clone <org>/<db>
cd <db>
dolt checkout -b fix-units
# edit…
dolt commit -am "normalise units to SI"
dolt push origin fix-units
# then open a PR on DoltHub
```

On the reviewer side, PR diffs are structural — you see cell-level changes,
not line-level file diffs. That is the point of Dolt over lakeFS/DVC: the
diff engine understands rows and columns.

## Workflow 4 — git-remote mode (Dolt v1.81.10+, Feb 2026)

New in Feb 2026: a Dolt database can use an ordinary Git repo (GitHub,
GitLab, local bare repo) as its remote. The DB data lives on a hidden
`refs/dolt/data` ref so `git clone` / `git fetch` ignore it — source code
and the DB coexist in one repo without collision.

Perfect fit for this project: source + nix + dolt database in one jj-managed
tree, one push target.

```bash
dolt init
dolt sql -q "create table t (pk int primary key);"
dolt sql -q "insert into t values (1), (2), (3);"
dolt add .
dolt commit -m "add table t and seed"

dolt remote add origin https://github.com/<org>/<repo>.git
dolt push origin main
```

From SQL (e.g. from an app or `dolt sql-server`):

```sql
call dolt_clone('https://github.com/<org>/<repo>.git');
use <repo>;
select * from t;

call dolt_commit('-Am', 'insert more data');
call dolt_push('origin', 'main');
```

Pull is plain CLI:

```bash
dolt pull origin main
```

**Implication for us:** the `mentci-next` GitHub repo can host both the
nix/source tree (pushed via `jj git push`) and a Dolt database (pushed via
`dolt push`) on the same remote URL, on separate refs.

## Workflow 5 — Dolt as agent memory

Dolt's pitch for this use case: agent writes/reads are transactional SQL;
every action produces a commit; multi-agent systems fork branches and merge
them; the whole history is inspectable and rewindable. Relevant ops:

- **Checkpoint per agent turn** — `call dolt_commit('-Am', 'turn N: <what>')`.
  Cheap; each turn becomes a revertible unit.
- **Per-agent branches** — branch per agent in a multi-agent run, merge at
  rendezvous points; conflicts = genuine disagreement about state.
- **Time travel for debugging** — `select ... as of 'HEAD~3'` runs the same
  query against a past snapshot without checkout.
- **Structural audit** — `select * from dolt_log` / `dolt_diff_<table>` is
  your audit log; no extra event table needed.

## Dolt vs the other "git for data" tools

| Tool | What it versions | When you pick it over Dolt |
|---|---|---|
| **lakeFS** | files/objects in S3-compatible storage | unstructured data; data lake, not OLTP |
| **DVC** | pipeline inputs + model artifacts | ML pipeline lineage + big binary artifacts |
| **Pachyderm** | pipeline executions | you want containerised data pipelines + lineage |
| **TerminusDB** | graph/document DB revisions | graph workloads; not SQL |
| **Dolt** | SQL tables at cell granularity | structured data you want to query + merge + diff |

Dolt is the only one that applies the Git model *literally* to relational
data: cell-level diffs, merges, conflicts.

## Running Dolt in this repo

Dolt isn't yet in the devshell. When/if we add it, the pattern is:

```nix
# devshell.nix
{ pkgs }:
pkgs.mkShell {
  packages = [ pkgs.dolt pkgs.mysql-client ];
}
```

`pkgs.dolt` exists in the pinned `criome/nixpkgs`. Then:

```bash
direnv allow
dolt version
dolt sql-server &          # MySQL-compat server on :3306
mysql -h127.0.0.1 -uroot   # connect with any MySQL client
```

## References

- Dolt source & docs — <https://github.com/dolthub/dolt>
- Git-for-data intro — <https://docs.dolthub.com/introduction/getting-started/git-for-data>
- Git-remote mode (Feb 2026) — <https://www.dolthub.com/blog/2026-02-13-announcing-git-remote-support-in-dolt/>
- Comparison writeup (2024) — <https://www.dolthub.com/blog/2024-09-24-git-for-data/>
- DoltHub — <https://www.dolthub.com/>

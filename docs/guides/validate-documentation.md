# Check documentation links locally

*Diataxis mode: how-to (repeatable documentation validation).*

Core includes a standard-library checker at `scripts/validate_docs.py`. It
checks Markdown links, image targets, case-sensitive local paths, heading
anchors, and GitHub blob/tree links for repositories supplied as local
checkouts. It makes no network requests. GitHub links without a mapped checkout
are reported as unchecked and still need a current source review.

## Check Core

From the Core repository root:

```powershell
python scripts/validate_docs.py --root .
```

## Check Workbench against Core

From a directory containing the Core and Workbench checkouts:

```powershell
python .\repopact-core\scripts\validate_docs.py `
  --root .\repopact-workbench `
  --repo ForgeWireLabs/repopact-core=.\repopact-core
```

Pass each repository whose GitHub links should resolve locally as another
`--repo OWNER/REPO=PATH` argument. This also checks exact filenames for
repository-split links to decisions, evidence, or source.

## Read the result correctly

The checker verifies structural targets. It cannot prove that a sentence still
matches implementation, a remote page is available, or an example's behavior
is correct. Execute documented commands and tutorials in disposable
repositories; compare version and platform statements with package metadata,
source, tests, and linked evidence. Record both passes and failures in the
governing Work Item's evidence.

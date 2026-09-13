#!/usr/bin/env python3

import argparse
import os
import pathlib
import plistlib
import shutil


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()

    source = args.source.resolve()
    output = args.output.resolve()
    if source.parent != output.parent:
        raise SystemExit("the xctestrun copy must stay beside its build products")
    if output.exists():
        raise SystemExit(f"refusing to replace {output}")

    shutil.copyfile(source, output)
    output.chmod(0o600)

    with output.open("rb") as stream:
        configuration = plistlib.load(stream)

    injected = {"WCASH_IOS_LIVE_REGTEST": "1"} if args.live else {}
    funded_seed = os.environ.get("WCASH_IOS_FUNDED_TEST_SEED")
    if funded_seed:
        injected["WCASH_IOS_FUNDED_TEST_SEED"] = funded_seed

    targets = 0
    for name, target in configuration.items():
        if name.startswith("__") or not isinstance(target, dict):
            continue
        if "TestBundlePath" not in target:
            continue
        targets += 1
        for key in ("EnvironmentVariables", "TestingEnvironmentVariables"):
            environment = target.setdefault(key, {})
            if not isinstance(environment, dict):
                raise SystemExit(f"{name}.{key} is not a dictionary")
            environment.pop("WCASH_IOS_LIVE_REGTEST", None)
            environment.pop("WCASH_IOS_FUNDED_TEST_SEED", None)
            environment.update(injected)

    if targets == 0:
        raise SystemExit("the xctestrun file contains no test targets")

    with output.open("wb") as stream:
        plistlib.dump(configuration, stream, fmt=plistlib.FMT_BINARY, sort_keys=False)


if __name__ == "__main__":
    main()

#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

test_name="ZingoTests"
set_test_name=false
run_live=false
destination="${WCASH_IOS_DESTINATION:-platform=iOS Simulator,name=iPhone 17,OS=latest}"

usage() {
    printf '%s\n' \
        "Run a previously built Wcash iOS XCTest bundle." \
        "" \
        "  -e TEST   Select a test target, class, or method (default: ZingoTests)" \
        "  -l        Run the Wcash Local Regtest class against 127.0.0.1:48234" \
        "  -h        Show this help"
}

while getopts ':e:lh' option; do
    case "$option" in
        e)
            test_name="$OPTARG"
            set_test_name=true
            ;;
        l)
            run_live=true
            ;;
        h)
            usage
            exit 0
            ;;
        :|?)
            usage >&2
            exit 2
            ;;
    esac
done

if [[ ! -d ios ]]; then
    printf 'Run %s from the wallet-mobile repository root.\n' "$0" >&2
    exit 2
fi

if ! xcodebuild -version >/dev/null 2>&1; then
    printf 'Full Xcode is required. Select it before running this script.\n' >&2
    exit 2
fi

if [[ "$run_live" == true ]]; then
    if [[ "$set_test_name" == false ]]; then
        if [[ -n "${WCASH_IOS_FUNDED_TEST_SEED:-}" ]]; then
            test_name="ZingoTests/WcashLocalRegtestTests"
        else
            test_name="ZingoTests/WcashLocalRegtestTests/testCreateRestoreSyncReceiveAndHistory"
        fi
    fi
    if ! nc -z -w 2 127.0.0.1 48234; then
        printf 'Wcash CompactTxStreamer is unavailable on 127.0.0.1:48234.\n' >&2
        exit 2
    fi
fi

xctestruns=()
while IFS= read -r path; do
    xctestruns+=("$path")
done < <(find ios/build/DerivedData/Build/Products -maxdepth 1 -type f -name 'Zingo_*.xctestrun' -print)
if [[ "${#xctestruns[@]}" -ne 1 ]]; then
    printf 'Expected one fresh Zingo xctestrun file; found %s.\n' "${#xctestruns[@]}" >&2
    exit 2
fi
xctestrun="${xctestruns[0]}"

selected_xctestrun="$xctestrun"
temporary_xctestrun=""
cleanup_xctestrun() {
    if [[ -n "$temporary_xctestrun" ]]; then
        rm -f -- "$temporary_xctestrun"
    fi
}
trap cleanup_xctestrun EXIT

if [[ "$run_live" == true ]]; then
    temporary_xctestrun="$(dirname "$xctestrun")/WcashLive-$$-$RANDOM.xctestrun"
    python3 scripts/ci/prepare-ios-xctestrun.py \
        --live \
        "$xctestrun" \
        "$temporary_xctestrun"
    selected_xctestrun="$temporary_xctestrun"
    unset WCASH_IOS_FUNDED_TEST_SEED
fi

report="ios/build/reports/Wcash-iOS-Test.xcresult"
test_log="ios/build/reports/Wcash-iOS-Test.log"
rm -rf "$report"
mkdir -p "$(dirname "$report")"

if ! xcodebuild test-without-building \
    -xctestrun "$selected_xctestrun" \
    -destination "$destination" \
    -parallel-testing-enabled NO \
    -resultBundlePath "$report" \
    -only-testing:"$test_name" 2>&1 | tee "$test_log"; then
    printf 'Wcash iOS integration tests failed.\n' >&2
    exit 1
fi

summary_arguments=()
if [[ "$run_live" == true ]]; then
    summary_arguments+=(--live)
fi
xcrun xcresulttool get test-results summary --path "$report" --compact |
    python3 scripts/ci/verify-ios-xcresult.py "${summary_arguments[@]}"

printf 'Wcash iOS integration tests passed.\n'

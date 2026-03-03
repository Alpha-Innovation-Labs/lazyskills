# Default: show help
default:
    @just help

help:
    @echo ""
    @echo "\033[1;36m================ lazyskills Commands ================\033[0m"
    @echo ""
    @echo "\033[1;35mDevelopment:\033[0m"
    @echo "  just \033[0;33mdev\033[0m      \033[0;32mRun the TUI locally\033[0m"
    @echo "  just \033[0;33mdemo-vhs\033[0m \033[0;32mPrepare and render VHS demo\033[0m"
    @echo "  just \033[0;33mdocs\033[0m     \033[0;32mRun docs app locally\033[0m"
    @echo ""
    @echo "\033[1;35mVerification:\033[0m"
    @echo "  just \033[0;33mcheck\033[0m    \033[0;32mCompile-check the project\033[0m"
    @echo "  just \033[0;33mdocs-sync-check\033[0m \033[0;32mCheck generated docs are current\033[0m"
    @echo ""
    @echo "\033[1;35mUtilities:\033[0m"
    @echo "  just \033[0;33mdocs-sync\033[0m \033[0;32mGenerate docs reference from Rust\033[0m"
    @echo "  just \033[0;33mdocs-gh-pub\033[0m \033[0;32mPublish docs to GitHub Pages\033[0m"
    @echo ""
    @echo "\033[1;35mRelease:\033[0m"
    @echo "  just \033[0;33mpub-bump\033[0m \033[0;32mBump patch version\033[0m"
    @echo "  just \033[0;33mpub\033[0m      \033[0;32mTag and publish release\033[0m"
    @echo ""

import 'justfiles/development/dev.just'
import 'justfiles/development/demo-vhs.just'
import 'justfiles/development/docs.just'
import 'justfiles/verification/check.just'
import 'justfiles/verification/docs-sync-check.just'
import 'justfiles/utilities/docs-sync.just'
import 'justfiles/utilities/docs-gh-pub.just'
import 'justfiles/utilities/pub-bump.just'
import 'justfiles/utilities/pub.just'

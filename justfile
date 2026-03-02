# Default: show help
default:
    @just help

help:
    @echo ""
    @echo "\033[1;36m================ skills-tui Commands ================\033[0m"
    @echo ""
    @echo "\033[1;35mDevelopment:\033[0m"
    @echo "  just \033[0;33mdev\033[0m      \033[0;32mRun the TUI locally\033[0m"
    @echo ""
    @echo "\033[1;35mVerification:\033[0m"
    @echo "  just \033[0;33mcheck\033[0m    \033[0;32mCompile-check the project\033[0m"
    @echo ""
    @echo "\033[1;35mRelease:\033[0m"
    @echo "  just \033[0;33mpub-bump\033[0m \033[0;32mBump patch version\033[0m"
    @echo "  just \033[0;33mpub\033[0m      \033[0;32mTag and publish release\033[0m"
    @echo ""

import 'justfiles/development/dev.just'
import 'justfiles/verification/check.just'
import 'justfiles/utilities/pub-bump.just'
import 'justfiles/utilities/pub.just'

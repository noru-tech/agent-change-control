# Terminal recording of the offline quick start, for the README.
#
# Render from the repository root with https://github.com/charmbracelet/vhs:
#
#   vhs docs/demo.tape
#
# The tape installs nothing: it expects `acc` on PATH (brew install noru-tech/tap/acc, or a
# release archive) and uses only the synthetic fixtures in tests/fixtures. No network, no token.

Output docs/assets/demo.gif

Require acc

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 720
Set Padding 20
Set TypingSpeed 40ms
Set Theme "GitHub Dark"

Hide
Type "export PS1='$ ' && clear"
Enter
Show

Type "acc --version"
Enter
Sleep 1s

Type "# An agent wrote the change and its operator approved it: one human, two logins."
Enter
Type "acc evaluate tests/fixtures/claude-operator-self-approved/events.json --format table"
Enter
Sleep 4s

Type "clear"
Enter
Type "# The same change, approved by a human independent of the operator."
Enter
Type "acc evaluate tests/fixtures/claude-clean/events.json --format table"
Enter
Sleep 4s

Type "clear"
Enter
Type "# Write the manifest, then enforce policy on it: exit 1 on a failing change."
Enter
Type "acc evaluate tests/fixtures/claude-operator-self-approved/events.json -q -o /tmp/acc-demo.json"
Enter
Sleep 1s
Type "acc check /tmp/acc-demo.json > /dev/null; echo exit $?"
Enter
Sleep 2s

Type "# An in-toto Statement re-validates: its subjects match and its findings recompute."
Enter
Type "acc validate tests/fixtures/claude-clean/expected.intoto.json"
Enter
Sleep 3s

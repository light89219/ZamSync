# ZamSync demo -- rendered with VHS (https://github.com/charmbracelet/vhs)
#
# Usage:
#   docker run --rm -v "${PWD}:/vhs" ghcr.io/charmbracelet/vhs demo.tape
#
# Output: demo.gif (committed to repo, embedded in README)

Output demo.gif

Set FontSize 13
Set Width 1000
Set Height 560
Set Theme "Dracula"
Set Padding 20
Set TypingSpeed 40ms

# ── Silent setup: download binary and prepare two nodes ──────────────────────
Hide
Type "export RUST_LOG=warn" Enter
Sleep 200ms
Type "curl -fsSL -o /usr/local/bin/zamsync https://github.com/Etoile-Bleu/ZamSync/releases/latest/download/zamsync-linux-x86_64 && chmod +x /usr/local/bin/zamsync" Enter
Sleep 12s
Type "zamsync keygen ./hub    2>/dev/null" Enter
Sleep 800ms
Type "zamsync keygen ./clinic 2>/dev/null" Enter
Sleep 800ms
Type "zamsync serve ./hub 127.0.0.1:7000 2>/dev/null &" Enter
Sleep 600ms
Show

# ── Step 1: collect events offline on the clinic node ────────────────────────
Type "# Collect patient events offline (clinic node)"
Sleep 800ms
Enter
Sleep 400ms

Type "zamsync submit ./clinic '{\"patient\": \"P-001\", \"ward\": \"3B\",  \"type\": \"admission\"}'"
Sleep 300ms
Enter
Sleep 300ms

Type "zamsync submit ./clinic '{\"patient\": \"P-002\", \"ward\": \"3B\",  \"type\": \"discharge\"}'"
Sleep 300ms
Enter
Sleep 300ms

Type "zamsync submit ./clinic '{\"patient\": \"P-003\", \"ward\": \"ICU\", \"type\": \"critical\"}'"
Sleep 300ms
Enter
Sleep 800ms

# ── Step 2: connection restored, sync to hub ──────────────────────────────────
Type ""
Enter
Type "# Connection restored -- sync to hub"
Sleep 800ms
Enter
Sleep 400ms

Type "zamsync sync ./clinic 127.0.0.1:7000 $(cat ./hub/.node_id)"
Sleep 300ms
Enter
Sleep 2500ms

# ── Step 3: verify hub received everything ────────────────────────────────────
Type ""
Enter
Type "# Hub received all events"
Sleep 800ms
Enter
Sleep 400ms

Type "zamsync info ./hub"
Sleep 300ms
Enter
Sleep 1500ms

Type ""
Enter
Type "zamsync audit ./hub --format json"
Sleep 300ms
Enter
Sleep 2000ms

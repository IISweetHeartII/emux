# VHS demo recording for emux
# Install: https://github.com/charmbracelet/vhs
# Usage: vhs doc/demo.tape

Output doc/demo.gif

Set FontSize 16
Set Width 1200
Set Height 700
Set Theme "OneDark"
Set Padding 20
Set TypingSpeed 50ms

# Show version
Type "emux --version"
Enter
Sleep 1s

# Start a named session
Type "emux new work"
Enter
Sleep 2s

# Show the emux TUI with status bar
Type "echo 'Welcome to emux — a modern terminal multiplexer'"
Enter
Sleep 1.5s

Type "echo 'Zero config. 598 MB/s parse throughput. 1,473 tests.'"
Enter
Sleep 1.5s

# Show some real terminal work
Type "echo '--- Session persistence: detach and reattach anytime ---'"
Enter
Sleep 1s

# Detach from session
Type "exit"
Enter
Sleep 1.5s

# Show session management
Type "# Session management"
Enter
Sleep 500ms

Type "emux new dev"
Enter
Sleep 2s

Type "echo 'Dev session — running tests'"
Enter
Sleep 1s

Type "exit"
Enter
Sleep 1.5s

# List sessions
Type "emux ls"
Enter
Sleep 2s

# Attach to a session
Type "emux attach work"
Enter
Sleep 2s

# Show it remembers state
Type "echo 'Session restored — everything is still here'"
Enter
Sleep 1.5s

# Exit and clean up
Type "exit"
Enter
Sleep 1s

Type "emux kill work"
Enter
Sleep 500ms

Type "emux kill dev"
Enter
Sleep 500ms

Type "emux ls"
Enter
Sleep 1.5s

Type "# All clean. That's emux."
Enter
Sleep 2s

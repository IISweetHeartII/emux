# VHS demo recording for emux
# Install: https://github.com/charmbracelet/vhs
# Usage: vhs doc/demo.tape

Output doc/demo.gif

Set FontSize 16
Set Width 1200
Set Height 700
Set Theme "One Dark"
Set Padding 20

# Start emux
Type "emux new demo"
Enter
Sleep 2s

# Show split panes
Type "echo 'Hello from emux!'"
Enter
Sleep 1s

# Split horizontally (Leader = Ctrl+Shift)
# Note: VHS uses Ctrl+Shift as key combo
Ctrl+Shift+d
Sleep 1s

Type "echo 'Bottom pane'"
Enter
Sleep 1s

# Split right
Ctrl+Shift+r
Sleep 1s

Type "echo 'Right pane'"
Enter
Sleep 1s

# Navigate between panes
Ctrl+Shift+Left
Sleep 500ms
Ctrl+Shift+Up
Sleep 500ms

# Create a new tab
Ctrl+Shift+t
Sleep 1s

Type "echo 'Tab 2'"
Enter
Sleep 1s

# Switch back to tab 1
Ctrl+Shift+p
Sleep 1s

# Detach
Ctrl+Shift+q
Sleep 1s

# Show session list
Type "emux ls"
Enter
Sleep 2s

# Reattach
Type "emux attach demo"
Enter
Sleep 2s

# Clean up
Ctrl+Shift+q
Sleep 500ms
Type "emux kill demo"
Enter
Sleep 1s

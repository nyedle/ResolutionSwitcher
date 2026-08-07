Unzip anywhere and run resolution-switcher.exe. Nothing gets installed and
nothing is written outside this folder.

portable.txt is what makes it portable. While it sits next to the program, your
settings live in config.json right here. Copy the whole folder to another PC
and you take everything with you.

Delete portable.txt and it uses the normal spot instead,
%APPDATA%\ResolutionSwitcher\config.json.

If you move the folder, turn "Start with Windows" off and on again so it points
at the right place.

https://github.com/nyedle/ResolutionSwitcher

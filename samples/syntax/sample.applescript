-- Adapted from HelgeSverre/tree-sitter-applescript (MIT).
(* Exercise comments, declarations, handlers, blocks, object specifiers,
   collections, and built-in values. *)

use AppleScript version "2.4"
use scripting additions

property greeting : "Hello"
property maxRetries : 3

global logEnabled
local tempValue

on greet(personName, given salutation:"Hi")
	set fullMessage to salutation & " " & personName & ", " & greeting & "!"
	if logEnabled is true then
		log fullMessage
	end if
	return fullMessage
end greet

repeat with i from 1 to maxRetries
	if i is greater than 1 then
		display dialog "Attempt " & i buttons {"OK", "Cancel"} default button "OK"
	end if
end repeat

tell application "Finder"
	set theFiles to every file of home whose size > 1000
	set firstName to name of item 1 of theFiles
end tell

set theResult to my greet("World", salutation:"Hey")

try
	set x to 10 / 0
on error errMsg number errNum
	log "Caught error " & errNum & ": " & errMsg
end try

set personRecord to {name:"Ada", age:36, active:true}
set numbers to {1, 2, 3, 4, 5}
set today to current date
set thing to missing value

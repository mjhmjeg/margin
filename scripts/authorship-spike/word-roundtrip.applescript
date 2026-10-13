-- A5 spike: open a .docx in Microsoft Word, optionally make scripted edits, save as .docx, close.
-- Word is driven only through its AppleScript object model: no keystrokes, no clicking.
--
-- usage: osascript word-roundtrip.applescript <in.docx> <out.docx> plain|edit bookmark|sdt|none
--   plain  open + save as + close
--   edit   also: insert inside AI span 1, delete the first word of AI span 2, type right after
--          AI span 3, copy paragraph 2 (all AI) to the end as formatted text (no clipboard).
--   anchor how a span is found: its hidden bookmark, its content control, or (none) by the
--          known offsets of the synthetic text.
-- Prints one note per edit step. A step that fails (a term this Word version lacks) is noted and
-- the save still happens. The first run may raise macOS's "… wants to control Microsoft Word"
-- automation prompt; if it is refused, osascript fails with error -1743 and nothing is saved.
--
-- Written from Word's published AppleScript dictionary; not yet run on this machine (Word is
-- not installed here).

on run argv
	if (count of argv) < 4 then error "usage: in.docx out.docx plain|edit bookmark|sdt|none"
	set inPath to item 1 of argv
	set outPath to item 2 of argv
	set mode to item 3 of argv
	set anchor to item 4 of argv
	set outHfs to (POSIX file outPath) as text
	set notes to {}
	tell application id "com.microsoft.Word"
		launch
		with timeout of 120 seconds
			open (POSIX file inPath)
			set doc to active document
			if mode is "edit" then set notes to my edits(doc, anchor)
			save as doc file name outHfs file format format document
			close doc saving no
		end timeout
	end tell
	return notes
end run

-- The text range of AI span k (1..3) in the synthetic document.
on spanRange(doc, anchor, k)
	tell application id "com.microsoft.Word"
		if anchor is "bookmark" then
			return text object of bookmark ("_MarginAI" & k) of doc
		else if anchor is "sdt" then
			return text object of content control k of doc
		else
			-- Offsets of the spans in the generated text (crates/docx/examples/authorship_spike.rs).
			if k is 1 then
				set ps to start of content of text object of paragraph 1 of doc
				return create range doc start (ps + 25) end (ps + 25 + 34)
			else if k is 2 then
				set r to text object of paragraph 2 of doc
				return create range doc start (start of content of r) end ((end of content of r) - 1)
			else
				set ps to start of content of text object of paragraph 3 of doc
				return create range doc start (ps + 20) end (ps + 20 + 25)
			end if
		end if
	end tell
end spanRange

on edits(doc, anchor)
	set notes to {}
	tell application id "com.microsoft.Word"
		-- 1. insert inside AI span 1, after "Mist lifts off the "
		try
			set r to my spanRange(doc, anchor, 1)
			set s to (start of content of r) + 19
			set content of (create range doc start s end s) to "wide "
			set end of notes to "1 inside: ok"
		on error msg
			set end of notes to "1 inside: " & msg
		end try
		-- 2. delete the first word of AI span 2 ("Seewis ")
		try
			set r to my spanRange(doc, anchor, 2)
			set s to start of content of r
			set content of (create range doc start s end (s + 7)) to ""
			set end of notes to "2 delete: ok"
		on error msg
			set end of notes to "2 delete: " & msg
		end try
		-- 3. type right after AI span 3
		try
			set r to my spanRange(doc, anchor, 3)
			set e to end of content of r
			set content of (create range doc start e end e) to " AFTER"
			set end of notes to "3 after: ok"
		on error msg
			set end of notes to "3 after: " & msg
		end try
		-- 4. copy paragraph 2 (all AI) to the end of the document, with formatting, no clipboard
		try
			set src to text object of paragraph 2 of doc
			set e to (end of content of text object of doc) - 1
			set formatted text of (create range doc start e end e) to formatted text of src
			set end of notes to "4 copy: ok"
		on error msg
			set end of notes to "4 copy: " & msg
		end try
	end tell
	return notes
end edits

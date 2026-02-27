A GitHub issue has been filed:

{{url}}

Title: {{title}}

{{body}}

Your task is to study this issue and produce a plan for addressing it.

1. Read and understand the issue. Consider whether it is valid: it may be misconceived, a duplicate,
   opened against the wrong repo, spam, etc. If so, say so and stop.
2. Reproduce the issue via a script written to a suitable in-repo location performing manual
   testing. The script should allow me to actually see the incorrect behavior of the application; it
   should not merely capture it and summarize it as e.g. "PASS"/"FAIL".  Additionally create an
   in-codebase test if possible. It is very important that this test FAILS: the application is
   behaving incorrectly therefore there is a bug in the test suite in the sense that at least one
   test should be failing but none are. Commit the failing test at this stage. If you're unable to
   reproduce, say so and stop.
3. Research the codebase thoroughly to understand the relevant code and design.
4. Enter planning mode and produce a concrete implementation plan. The plan must conclude with a
   draft of the final message that will be delivered to me, with placeholder sections for what has
   been done, and a final section instructing me exactly how to verify the change. Here, "verify"
   means (1) to manually repro it before the code change, (2) to confirm that the tests fail on the
   commit prior to the fix, and (3) to confirm that the fix works in the sens that the tests now
   pass and the repro shows correct behavior.

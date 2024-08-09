@REM needs fd-find, rargs and ronfmt to be installed
fd chart.ron$ demo | rargs ronfmt {0}
fd project.ron demo | rargs ronfmt {0}
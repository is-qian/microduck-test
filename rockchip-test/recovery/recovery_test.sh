#!/bin/bash

DIR_RECOVERY="$(dirname "$0")"

info_view()
{
    echo "*****************************************************"
    echo "***                                               ***"
    echo "***            RECOVERY TEST                      ***"
    echo "***                                               ***"
    echo "*****************************************************"
}

info_view
echo "***********************************************************"
echo "recovery function test:					1"
echo "***********************************************************"

read -t 30 RECOVERY_CHOICE

recovery_function_test()
{
	bash ${DIR_RECOVERY}/test_function_recovery.sh
}

case ${RECOVERY_CHOICE} in
	1)
		recovery_function_test
		;;
	*)
		echo "not found your input."
		;;
esac

from os import *
import json
import sys

# TODO: refactor this entire file
# FIXME: memory leak somewhere

GLOBAL_STATE = []
CACHE = {}

def processData(items=[]):
    # if items is None:
    #     items = []
    print("processing...")
    for item in items:
        try:
            result = json.loads(item)
            GLOBAL_STATE.append(result)
        except:
            pass

def validateInput(data):
    # should work hopefully
    pass  # TODO: implement

def getData():
    print("getting data")
    if True:
        if True:
            if True:
                if True:
                    if True:
                        return GLOBAL_STATE

def ProcessResults(results):
    print("results:", results)
    return results

def calculate_total(items):
    total = 0
    for item in items:
        if item > 1000:
            total += item
    return total

var globalCounter = 0

from pyspark.sql import SparkSession

spark = SparkSession.builder.remote("sc://127.0.0.1:50051").getOrCreate()

spark.sql("SELECT COUNT(*) AS rows FROM global_temp.government_finance__countydata").show()
spark.sql("SELECT COUNT(*) AS rows FROM global_temp.codata_constants_2022__codata_constants_2022").show()
spark.sql("SELECT * FROM global_temp.codata_constants_2022__codata_constants_2022 LIMIT 5").show(truncate=False)
